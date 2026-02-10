// ============================================================================
// File: BlockShadowState.cs
// Description: Local shadow state for block editor content. Acts as the single
//              source of truth while a page is open. The plugin becomes an
//              async persistence backend — changes apply locally first (instant,
//              never lost), then drain to the plugin in the background.
//
//              All access MUST be on the UI thread — no locks needed.
// ============================================================================

using System.Text;
using System.Text.Json;
using Serilog;

namespace PrivStack.UI.Adaptive;

// -- Content types (mirrors Rust BlockContent enum) --

public abstract record ShadowBlockContent;
public sealed record TextContent(string Text) : ShadowBlockContent;
public sealed record HeadingContent(int Level, string Text) : ShadowBlockContent;
public sealed record CodeContent(string Code, string Language) : ShadowBlockContent;
public sealed record BlockquoteContent(string Text) : ShadowBlockContent;
public sealed record CalloutContent(string Icon, string Variant, string Text) : ShadowBlockContent;
public sealed record ListContent(bool Ordered, List<ShadowListItem> Items) : ShadowBlockContent;
public sealed record TaskListContent(List<ShadowListItem> Items) : ShadowBlockContent;
public sealed record ImageContent(string Url, string? Alt, string? Title, string Align = "left", double? Width = null) : ShadowBlockContent;
public sealed record HorizontalRuleContent() : ShadowBlockContent;
public sealed record FootnoteContent(string Label, string Content) : ShadowBlockContent;
public sealed record DefinitionListContent(List<ShadowDefinitionItem> Items) : ShadowBlockContent;
public sealed record TableContent(List<string> Columns, ShadowTableRow? HeaderRow, List<ShadowTableRow> Rows, bool ShowHeader = true, bool AlternatingRows = false, List<double>? ColumnWidths = null) : ShadowBlockContent;
public sealed record TableOfContentsContent(string Mode) : ShadowBlockContent;

public sealed record ShadowListItem(string Id, string Text, bool IsChecked, List<ShadowListItem> Children);
public sealed record ShadowDefinitionItem(string Id, string Term, List<ShadowDefinitionEntry> Definitions);
public sealed record ShadowDefinitionEntry(string Id, string Text);
public sealed record ShadowTableRow(string Id, List<ShadowTableCell> Cells);
public sealed record ShadowTableCell(string Id, string Text);

public sealed class ShadowBlock
{
    public string Id { get; }
    public string Type { get; }
    public ShadowBlockContent Content { get; set; }

    // Extra metadata for rendering
    public int Level { get; set; }
    public string Language { get; set; } = "";
    public string Icon { get; set; } = "";
    public string Variant { get; set; } = "";
    public string Layout { get; set; } = "single";
    public string? PairId { get; set; }
    public string PairValign { get; set; } = "top";

    public ShadowBlock(string id, string type, ShadowBlockContent content)
    {
        Id = id;
        Type = type;
        Content = content;
    }
}

public sealed record PendingCommand(long Seq, string Command, string ArgsJson, DateTimeOffset QueuedAt);

public sealed class BlockShadowState
{
    private static readonly ILogger _log = Log.ForContext<BlockShadowState>();

    private readonly List<ShadowBlock> _blocks = new();
    private readonly Queue<PendingCommand> _pendingCommands = new();
    private long _nextSeq;

    public string? PageId { get; private set; }
    public IReadOnlyList<ShadowBlock> Blocks => _blocks;
    public bool IsDirty => _pendingCommands.Count > 0;
    public bool HasBlocks => _blocks.Count > 0;

    // ================================================================
    // Lifecycle
    // ================================================================

    /// <summary>
    /// Populate shadow state from plugin JSON on page load.
    /// </summary>
    public void LoadFromPluginJson(string pageId, JsonElement blocksArray)
    {
        Clear();
        PageId = pageId;

        if (blocksArray.ValueKind != JsonValueKind.Array) return;

        foreach (var block in blocksArray.EnumerateArray())
        {
            var id = block.GetStringProp("id") ?? "";
            var type = block.GetStringProp("type") ?? "paragraph";
            var sb = ParseBlock(block, type);
            _blocks.Add(sb);
        }

        // Detect duplicate IDs — plugin bug, but we handle it
        var dupes = _blocks.GroupBy(b => b.Id).Where(g => g.Count() > 1).Select(g => g.Key).ToList();
        if (dupes.Count > 0)
            _log.Warning("Shadow: DUPLICATE block IDs from plugin: [{Dupes}]. Using type-aware lookup to disambiguate.",
                string.Join(", ", dupes));

        _log.Debug("Shadow: Loaded {Count} blocks for page {PageId}. IDs: [{Ids}]",
            _blocks.Count, pageId, string.Join(", ", _blocks.Select(b => $"{b.Id}({b.Type})")));
    }

    /// <summary>
    /// Drain all pending commands synchronously (before save/page switch).
    /// </summary>
    public void FlushSync(Action<string, string> sendCommandSilent)
    {
        while (_pendingCommands.Count > 0)
        {
            var cmd = _pendingCommands.Dequeue();
            var age = DateTimeOffset.UtcNow - cmd.QueuedAt;
            _log.Debug("Shadow: Flushing seq={Seq} cmd={Cmd} (queued {Age}ms ago)",
                cmd.Seq, cmd.Command, (int)age.TotalMilliseconds);
            sendCommandSilent(cmd.Command, cmd.ArgsJson);
            _log.Debug("Shadow: ACK seq={Seq}", cmd.Seq);
        }
    }

    /// <summary>
    /// Drain pending commands (called by timer every 300ms).
    /// Returns true if any commands were sent.
    /// </summary>
    public bool DrainPendingCommands(Action<string, string> sendCommandSilent)
    {
        var sent = false;
        while (_pendingCommands.Count > 0)
        {
            sent = true;
            var cmd = _pendingCommands.Dequeue();
            var age = DateTimeOffset.UtcNow - cmd.QueuedAt;
            _log.Debug("Shadow: Sending seq={Seq} cmd={Cmd} (queued {Age}ms ago)",
                cmd.Seq, cmd.Command, (int)age.TotalMilliseconds);
            sendCommandSilent(cmd.Command, cmd.ArgsJson);
            _log.Debug("Shadow: ACK seq={Seq}", cmd.Seq);
        }
        return sent;
    }

    /// <summary>
    /// Merge blocks from plugin JSON that don't exist in shadow state.
    /// This handles the case where the plugin creates a new block (e.g. via palette)
    /// and a re-render occurs — shadow state must absorb the new block.
    /// Existing shadow blocks are preserved (they may have unsaved edits).
    /// </summary>
    /// <summary>
    /// Merges new blocks from plugin JSON into shadow state.
    /// Returns the IDs of newly added blocks (if any).
    /// </summary>
    public List<string> MergeNewBlocksFromPlugin(JsonElement pluginBlocksArray)
    {
        if (pluginBlocksArray.ValueKind != JsonValueKind.Array) return [];

        // Build a set of all IDs+types already in shadow
        var existingKeys = new HashSet<string>();
        foreach (var b in _blocks)
            existingKeys.Add($"{b.Id}:{b.Type}");

        // Walk plugin blocks in order; insert any that shadow doesn't have
        var pluginBlocks = new List<(int pluginIdx, ShadowBlock block)>();
        var pluginIdx = 0;
        foreach (var blockEl in pluginBlocksArray.EnumerateArray())
        {
            var id = blockEl.GetStringProp("id") ?? "";
            var type = blockEl.GetStringProp("type") ?? "paragraph";
            var key = $"{id}:{type}";
            if (!existingKeys.Contains(key))
            {
                var sb = ParseBlock(blockEl, type);
                pluginBlocks.Add((pluginIdx, sb));
                _log.Debug("Shadow: Merging new block from plugin — id={Id} type={Type} at pluginIdx={Idx}",
                    id, type, pluginIdx);
            }
            pluginIdx++;
        }

        // Insert new blocks at approximately correct positions
        foreach (var (pIdx, newBlock) in pluginBlocks)
        {
            if (pIdx >= _blocks.Count)
                _blocks.Add(newBlock);
            else
                _blocks.Insert(pIdx, newBlock);
        }

        return pluginBlocks.Select(p => p.block.Id).ToList();
    }

    public void Clear()
    {
        _log.Debug("Shadow: Clear() — dropping {BlockCount} blocks, {PendingCount} pending commands, pageId={PageId}",
            _blocks.Count, _pendingCommands.Count, PageId);
        _blocks.Clear();
        _pendingCommands.Clear();
        _nextSeq = 0;
        PageId = null;
    }

    // ================================================================
    // Mutations — each updates _blocks + enqueues PendingCommand
    // ================================================================

    public void UpdateBlockText(string blockId, string text)
    {
        var block = GetBlock(blockId);
        if (block is null)
        {
            _log.Warning("Shadow: UpdateBlockText MISS — blockId={BlockId} not found. Known IDs: [{Ids}]",
                blockId, string.Join(", ", _blocks.Select(b => b.Id)));
            return;
        }

        var contentType = block.Content.GetType().Name;
        block.Content = block.Content switch
        {
            TextContent => new TextContent(text),
            HeadingContent h => new HeadingContent(h.Level, text),
            BlockquoteContent => new BlockquoteContent(text),
            CalloutContent c => new CalloutContent(c.Icon, c.Variant, text),
            FootnoteContent f => new FootnoteContent(f.Label, text),
            _ => block.Content,
        };

        _log.Debug("Shadow: UpdateBlockText blockId={BlockId} contentType={ContentType} textLen={Len} pending={Pending}",
            blockId, contentType, text.Length, _pendingCommands.Count + 1);

        if (block.Content is FootnoteContent fn2)
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, content = text }));
        else
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, text }));
    }

    public void UpdateBlockCode(string blockId, string code)
    {
        var block = GetBlock(blockId);
        if (block?.Content is CodeContent cc)
        {
            block.Content = new CodeContent(code, cc.Language);
            _log.Debug("Shadow: UpdateBlockCode blockId={BlockId} codeLen={Len}", blockId, code.Length);
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, code }));
        }
        else
        {
            _log.Warning("Shadow: UpdateBlockCode MISS — blockId={BlockId} block={Found} contentType={Type}",
                blockId, block is not null, block?.Content.GetType().Name ?? "null");
        }
    }

    public void UpdateBlockLanguage(string blockId, string language)
    {
        var block = GetBlock(blockId);
        if (block?.Content is CodeContent cc)
        {
            block.Content = new CodeContent(cc.Code, language);
            block.Language = language;
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, language }));
        }
    }

    public void UpdateListItemText(string blockId, string itemId, string text)
    {
        var block = GetListBlock(blockId);
        if (block is null)
        {
            _log.Warning("Shadow: UpdateListItemText MISS — blockId={BlockId} not found. Known IDs: [{Ids}]",
                blockId, string.Join(", ", _blocks.Select(b => $"{b.Id}({b.Content.GetType().Name})")));
            return;
        }

        var items = block.Content switch
        {
            ListContent lc => lc.Items,
            TaskListContent tc => tc.Items,
            _ => null,
        };
        if (items is null)
        {
            _log.Warning("Shadow: UpdateListItemText — block {BlockId} is {Type}, not a list (even after GetListBlock)",
                blockId, block.Content.GetType().Name);
            return;
        }

        var item = FindListItem(items, itemId);
        if (item is not null)
        {
            ReplaceListItem(items, itemId, item with { Text = text });
            _log.Debug("Shadow: UpdateListItemText blockId={BlockId} itemId={ItemId} textLen={Len}",
                blockId, itemId, text.Length);
        }
        else
        {
            _log.Warning("Shadow: UpdateListItemText — itemId={ItemId} not found in block {BlockId}. Known item IDs: [{Ids}]",
                itemId, blockId, string.Join(", ", FlattenItemIds(items)));
        }

        Enqueue("update_list_item", JsonSerializer.Serialize(
            new { id = blockId, item_id = itemId, text }));
    }

    public string AddBlock(string? afterBlockId, string newBlockId, string type, string text)
    {
        var newBlock = new ShadowBlock(newBlockId, type, new TextContent(text));

        if (afterBlockId is null || _blocks.Count == 0)
        {
            _blocks.Add(newBlock);
        }
        else
        {
            var idx = _blocks.FindIndex(b => b.Id == afterBlockId);
            if (idx >= 0)
                _blocks.Insert(idx + 1, newBlock);
            else
                _blocks.Add(newBlock);
        }

        Enqueue("split_block", JsonSerializer.Serialize(
            new { id = afterBlockId ?? "", after_text = text, new_block_id = newBlockId }));

        return newBlockId;
    }

    public void AddBlockRaw(string? afterBlockId, string newBlockId, string type, string text)
    {
        var content = CreateContent(type, text, 1, "", "", "", false);
        var newBlock = new ShadowBlock(newBlockId, type, content);

        if (afterBlockId is null || _blocks.Count == 0)
        {
            _blocks.Add(newBlock);
        }
        else
        {
            var idx = _blocks.FindIndex(b => b.Id == afterBlockId);
            if (idx >= 0)
                _blocks.Insert(idx + 1, newBlock);
            else
                _blocks.Add(newBlock);
        }
        // No enqueue — caller handles the command
    }

    public void ConvertBlock(string blockId, string newType, int level = 1)
    {
        var block = GetBlock(blockId);
        if (block is null) return;

        var text = GetBlockText(block);
        block.Content = newType switch
        {
            "paragraph" => new TextContent(text),
            "heading" => new HeadingContent(level, text),
            "blockquote" => new BlockquoteContent(text),
            "callout" => new CalloutContent("📢", "", text),
            "bullet_list" => new ListContent(false, [new ShadowListItem(Guid.NewGuid().ToString("N")[..8], text, false, [])]),
            "numbered_list" => new ListContent(true, [new ShadowListItem(Guid.NewGuid().ToString("N")[..8], text, false, [])]),
            "task_list" => new TaskListContent([new ShadowListItem(Guid.NewGuid().ToString("N")[..8], text, false, [])]),
            _ => block.Content,
        };

        // Update metadata
        var oldType = block.Type;
        // Type is readonly on ShadowBlock — we need to replace in list
        var idx = _blocks.IndexOf(block);
        if (idx >= 0)
        {
            var newBlock = new ShadowBlock(block.Id, newType, block.Content) { Level = level };
            _blocks[idx] = newBlock;
        }

        Enqueue("convert_block", JsonSerializer.Serialize(new { id = blockId, new_type = newType, level }));
    }

    public void DeleteBlock(string blockId)
    {
        _blocks.RemoveAll(b => b.Id == blockId);
        Enqueue("delete_block", JsonSerializer.Serialize(new { id = blockId }));
    }

    public void SplitBlock(string blockId, string afterText, string newBlockId)
    {
        var block = GetBlock(blockId);
        if (block is null) return;

        // Trim current block text to before-cursor content
        var currentText = GetBlockText(block);
        var splitIdx = currentText.Length - afterText.Length;
        if (splitIdx > 0)
        {
            var beforeText = currentText[..splitIdx];
            SetBlockText(block, beforeText);
        }

        // Insert new paragraph after current
        var newBlock = new ShadowBlock(newBlockId, "paragraph", new TextContent(afterText));
        var idx = _blocks.IndexOf(block);
        if (idx >= 0)
            _blocks.Insert(idx + 1, newBlock);
        else
            _blocks.Add(newBlock);

        Enqueue("split_block", JsonSerializer.Serialize(
            new { id = blockId, after_text = afterText, new_block_id = newBlockId }));
    }

    public void MergeBlockWithPrevious(string blockId)
    {
        var idx = _blocks.FindIndex(b => b.Id == blockId);
        if (idx <= 0) return;

        var current = _blocks[idx];
        var prev = _blocks[idx - 1];

        // Merge text
        var prevText = GetBlockText(prev);
        var currText = GetBlockText(current);
        SetBlockText(prev, prevText + currText);

        _blocks.RemoveAt(idx);

        Enqueue("merge_block_with_previous", JsonSerializer.Serialize(new { id = blockId }));
    }

    public void ReorderBlock(string blockId, string targetId, string position)
    {
        var srcIdx = _blocks.FindIndex(b => b.Id == blockId);
        if (srcIdx < 0) return;

        var block = _blocks[srcIdx];
        _blocks.RemoveAt(srcIdx);

        var tgtIdx = _blocks.FindIndex(b => b.Id == targetId);
        if (tgtIdx < 0)
        {
            _blocks.Add(block);
        }
        else
        {
            var insertIdx = position == "after" ? tgtIdx + 1 : tgtIdx;
            _blocks.Insert(insertIdx, block);
        }

        Enqueue("reorder_block", JsonSerializer.Serialize(
            new { id = blockId, target_id = targetId, position }));
    }

    public string AddListItem(string blockId, string afterItemId, string newItemId, string text)
    {
        var block = GetListBlock(blockId);
        if (block is null)
        {
            _log.Warning("Shadow: AddListItem MISS — blockId={BlockId} not found", blockId);
            return newItemId;
        }

        var items = block.Content switch
        {
            ListContent lc => lc.Items,
            TaskListContent tc => tc.Items,
            _ => null,
        };
        if (items is null) return newItemId;

        var newItem = new ShadowListItem(newItemId, text, false, new List<ShadowListItem>());
        InsertListItemAfter(items, afterItemId, newItem);

        Enqueue("add_list_item", JsonSerializer.Serialize(
            new { id = blockId, after_item_id = afterItemId, text, new_item_id = newItemId }));

        return newItemId;
    }

    public void ToggleTaskItem(string blockId, string itemId)
    {
        var block = GetListBlock(blockId);
        if (block?.Content is not TaskListContent tc) return;

        var item = FindListItem(tc.Items, itemId);
        if (item is not null)
        {
            ReplaceListItem(tc.Items, itemId, item with { IsChecked = !item.IsChecked });
        }

        Enqueue("toggle_task_item", JsonSerializer.Serialize(
            new { id = blockId, item_id = itemId }));
    }

    public void IndentListItem(string blockId, string itemId)
    {
        Enqueue("indent_list_item", JsonSerializer.Serialize(
            new { id = blockId, item_id = itemId }));

        // Mirror the tree change in shadow state so markdown view stays in sync
        var block = _blocks.FirstOrDefault(b => b.Id == blockId);
        if (block?.Content is ListContent lc)
            IndentShadowItem(lc.Items, itemId);
        else if (block?.Content is TaskListContent tc)
            IndentShadowItem(tc.Items, itemId);
    }

    public void OutdentListItem(string blockId, string itemId)
    {
        Enqueue("outdent_list_item", JsonSerializer.Serialize(
            new { id = blockId, item_id = itemId }));

        var block = _blocks.FirstOrDefault(b => b.Id == blockId);
        if (block?.Content is ListContent lc)
            OutdentShadowItem(lc.Items, itemId);
        else if (block?.Content is TaskListContent tc)
            OutdentShadowItem(tc.Items, itemId);
    }

    /// <summary>Move item under its previous sibling (same logic as Rust indent).</summary>
    private static bool IndentShadowItem(List<ShadowListItem> items, string itemId)
    {
        for (var i = 0; i < items.Count; i++)
        {
            if (items[i].Id == itemId && i > 0)
            {
                var item = items[i];
                items.RemoveAt(i);
                items[i - 1].Children.Add(item);
                return true;
            }
            if (IndentShadowItem(items[i].Children, itemId)) return true;
        }
        return false;
    }

    /// <summary>Move item from parent's children up one level (same logic as Rust outdent).</summary>
    private static bool OutdentShadowItem(List<ShadowListItem> items, string itemId)
    {
        for (var i = 0; i < items.Count; i++)
        {
            var idx = items[i].Children.FindIndex(c => c.Id == itemId);
            if (idx >= 0)
            {
                var item = items[i].Children[idx];
                items[i].Children.RemoveAt(idx);
                items.Insert(i + 1, item);
                return true;
            }
            if (OutdentShadowItem(items[i].Children, itemId)) return true;
        }
        return false;
    }

    // ================================================================
    // Query
    // ================================================================

    public ShadowBlock? GetBlock(string blockId) =>
        _blocks.FirstOrDefault(b => b.Id == blockId);

    /// <summary>
    /// Find a block by ID, preferring one whose content is a list type.
    /// Handles duplicate IDs from plugins (e.g. same ID for a paragraph and a list).
    /// </summary>
    public ShadowBlock? GetListBlock(string blockId)
    {
        ShadowBlock? first = null;
        foreach (var b in _blocks)
        {
            if (b.Id != blockId) continue;
            if (b.Content is ListContent or TaskListContent) return b;
            first ??= b;
        }
        return first; // fallback if no list match
    }

    /// <summary>
    /// Find a block by ID, preferring one whose content matches the expected type.
    /// </summary>
    public ShadowBlock? GetBlock(string blockId, Type expectedContentType)
    {
        ShadowBlock? first = null;
        foreach (var b in _blocks)
        {
            if (b.Id != blockId) continue;
            if (b.Content.GetType() == expectedContentType) return b;
            first ??= b;
        }
        return first;
    }

    /// <summary>
    /// Serialize shadow blocks to a JSON array that can be fed to the existing
    /// RenderBlock path (same shape as plugin JSON).
    /// </summary>
    public string? SerializeSingleBlockJson(string blockId)
    {
        var block = GetBlock(blockId);
        return block is null ? null : SerializeBlock(block);
    }

    public string SerializeBlocksJson()
    {
        var sb = new StringBuilder("[");
        for (var i = 0; i < _blocks.Count; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append(SerializeBlock(_blocks[i]));
        }
        sb.Append(']');
        return sb.ToString();
    }

    /// <summary>
    /// Build full markdown from shadow state (replaces BuildMarkdownFromLiveBlocks).
    /// </summary>
    public string BuildMarkdown()
    {
        var sb = new StringBuilder();
        foreach (var block in _blocks)
        {
            switch (block.Content)
            {
                case TextContent tc:
                    sb.AppendLine(tc.Text);
                    break;
                case HeadingContent hc:
                    sb.Append(new string('#', hc.Level));
                    sb.Append(' ');
                    sb.AppendLine(hc.Text);
                    break;
                case CodeContent cc:
                    sb.AppendLine($"```{cc.Language}");
                    sb.AppendLine(cc.Code);
                    sb.AppendLine("```");
                    break;
                case BlockquoteContent bq:
                    foreach (var line in bq.Text.Split('\n'))
                        sb.AppendLine($"> {line}");
                    break;
                case CalloutContent co:
                    sb.AppendLine($"> {co.Icon} {co.Text}");
                    break;
                case ListContent lc:
                    AppendListItemsMd(sb, lc.Items, lc.Ordered, 0);
                    break;
                case TaskListContent tc:
                    AppendTaskItemsMd(sb, tc.Items, 0);
                    break;
                case ImageContent ic:
                    sb.AppendLine($"![{ic.Alt ?? ""}]({ic.Url})");
                    break;
                case HorizontalRuleContent:
                    sb.AppendLine("---");
                    break;
                case FootnoteContent fn:
                    sb.AppendLine($"[^{fn.Label}]: {fn.Content}");
                    break;
                case DefinitionListContent dl:
                    foreach (var item in dl.Items)
                    {
                        sb.AppendLine(item.Term);
                        foreach (var def in item.Definitions)
                            sb.AppendLine($": {def.Text}");
                    }
                    break;
                case TableContent tc:
                    var colCount = tc.Columns.Count;
                    if (tc.HeaderRow is not null)
                    {
                        sb.AppendLine("| " + string.Join(" | ", tc.HeaderRow.Cells.Select(c => c.Text)) + " |");
                        sb.AppendLine("| " + string.Join(" | ", tc.Columns.Select(a => a switch
                        {
                            "center" => ":---:",
                            "right" => "---:",
                            _ => "---",
                        })) + " |");
                    }
                    foreach (var row in tc.Rows)
                        sb.AppendLine("| " + string.Join(" | ", row.Cells.Select(c => c.Text)) + " |");
                    break;
                case TableOfContentsContent toc:
                    sb.AppendLine($"[TOC:{toc.Mode}]");
                    break;
            }
            sb.AppendLine();
        }
        return sb.ToString().TrimEnd();
    }

    // ================================================================
    // Internals
    // ================================================================

    private void Enqueue(string command, string argsJson)
    {
        var seq = _nextSeq++;
        _pendingCommands.Enqueue(new PendingCommand(seq, command, argsJson, DateTimeOffset.UtcNow));
    }

    private static ShadowBlock ParseBlock(JsonElement block, string type)
    {
        var id = block.GetStringProp("id") ?? "";
        var level = block.GetIntProp("level", 1);
        var language = block.GetStringProp("language") ?? "";
        var icon = block.GetStringProp("icon") ?? "";
        var variant = block.GetStringProp("variant") ?? "";
        var text = block.GetStringProp("text") ?? "";
        var code = block.GetStringProp("code") ?? block.GetStringProp("text") ?? "";
        var ordered = type == "numbered_list";

        var content = CreateContent(type, text, level, code, language, icon, ordered, variant, block);

        var layout = block.GetStringProp("layout") ?? "single";
        var pairId = block.GetStringProp("pair_id");
        var pairValign = block.GetStringProp("pair_valign") ?? "top";

        var sb = new ShadowBlock(id, type, content)
        {
            Level = level,
            Language = language,
            Icon = icon,
            Variant = variant,
            Layout = layout == "side_by_side" ? "side_by_side" : "single",
            PairId = pairId,
            PairValign = pairValign,
        };
        return sb;
    }

    private static ShadowBlockContent CreateContent(
        string type, string text, int level, string code, string language,
        string icon, bool ordered, string variant = "", JsonElement block = default)
    {
        return type switch
        {
            "paragraph" => new TextContent(text),
            "heading" => new HeadingContent(level, text),
            "code_block" => new CodeContent(code, language),
            "blockquote" => new BlockquoteContent(text),
            "callout" => new CalloutContent(icon, variant, text),
            "bullet_list" => new ListContent(false, ParseListItems(block)),
            "numbered_list" => new ListContent(true, ParseListItems(block)),
            "task_list" => new TaskListContent(ParseListItems(block)),
            "image" => new ImageContent(
                block.ValueKind == JsonValueKind.Object ? block.GetStringProp("url") ?? "" : "",
                block.ValueKind == JsonValueKind.Object ? block.GetStringProp("alt") : null,
                block.ValueKind == JsonValueKind.Object ? block.GetStringProp("title") : null,
                block.ValueKind == JsonValueKind.Object ? block.GetStringProp("align") ?? "left" : "left",
                block.ValueKind == JsonValueKind.Object && block.TryGetProperty("width", out var imgW) && imgW.ValueKind == JsonValueKind.Number ? imgW.GetDouble() : null),
            "horizontal_rule" => new HorizontalRuleContent(),
            "footnote" => new FootnoteContent(
                block.ValueKind == JsonValueKind.Object ? block.GetStringProp("label") ?? "1" : "1",
                block.ValueKind == JsonValueKind.Object ? block.GetStringProp("content") ?? text : text),
            "definition_list" => new DefinitionListContent(ParseDefinitionItems(block)),
            "table" => new TableContent(ParseColumnAlignments(block), ParseHeaderRow(block), ParseTableRows(block),
                block.ValueKind == JsonValueKind.Object && block.TryGetProperty("show_header", out var sh) ? sh.GetBoolean() : true,
                block.ValueKind == JsonValueKind.Object && block.TryGetProperty("alternating_rows", out var ar) && ar.GetBoolean(),
                ParseColumnWidths(block)),
            "table_of_contents" => new TableOfContentsContent(
                block.ValueKind == JsonValueKind.Object ? block.GetStringProp("mode") ?? "document" : "document"),
            _ => new TextContent(text),
        };
    }

    private static List<ShadowListItem> ParseListItems(JsonElement block)
    {
        var result = new List<ShadowListItem>();
        if (block.ValueKind != JsonValueKind.Object) return result;
        if (!block.TryGetProperty("items", out var items) || items.ValueKind != JsonValueKind.Array)
            return result;

        foreach (var item in items.EnumerateArray())
        {
            result.Add(ParseListItem(item));
        }
        return result;
    }

    private static ShadowListItem ParseListItem(JsonElement item)
    {
        var text = item.ValueKind == JsonValueKind.String
            ? item.GetString() ?? ""
            : item.GetStringProp("text") ?? "";
        var id = item.GetStringProp("id") ?? "";
        var isChecked = item.GetBoolProp("checked", false) || item.GetBoolProp("is_checked", false);
        var children = new List<ShadowListItem>();

        if (item.TryGetProperty("children", out var ch) && ch.ValueKind == JsonValueKind.Array)
        {
            foreach (var child in ch.EnumerateArray())
                children.Add(ParseListItem(child));
        }

        return new ShadowListItem(id, text, isChecked, children);
    }

    private static ShadowListItem? FindListItem(List<ShadowListItem> items, string id)
    {
        foreach (var item in items)
        {
            if (item.Id == id) return item;
            var found = FindListItem(item.Children, id);
            if (found is not null) return found;
        }
        return null;
    }

    private static bool ReplaceListItem(List<ShadowListItem> items, string id, ShadowListItem replacement)
    {
        for (var i = 0; i < items.Count; i++)
        {
            if (items[i].Id == id) { items[i] = replacement; return true; }
            if (ReplaceListItem(items[i].Children, id, replacement)) return true;
        }
        return false;
    }

    private static bool InsertListItemAfter(List<ShadowListItem> items, string afterId, ShadowListItem newItem)
    {
        for (var i = 0; i < items.Count; i++)
        {
            if (items[i].Id == afterId)
            {
                items.Insert(i + 1, newItem);
                return true;
            }
            if (InsertListItemAfter(items[i].Children, afterId, newItem)) return true;
        }
        return false;
    }

    private static IEnumerable<string> FlattenItemIds(List<ShadowListItem> items)
    {
        foreach (var item in items)
        {
            yield return item.Id;
            foreach (var childId in FlattenItemIds(item.Children))
                yield return childId;
        }
    }

    private static string GetBlockText(ShadowBlock block) => block.Content switch
    {
        TextContent tc => tc.Text,
        HeadingContent hc => hc.Text,
        BlockquoteContent bq => bq.Text,
        CalloutContent co => co.Text,
        FootnoteContent fn => fn.Content,
        _ => "",
    };

    private static void SetBlockText(ShadowBlock block, string text)
    {
        block.Content = block.Content switch
        {
            TextContent => new TextContent(text),
            HeadingContent h => new HeadingContent(h.Level, text),
            BlockquoteContent => new BlockquoteContent(text),
            CalloutContent c => new CalloutContent(c.Icon, c.Variant, text),
            FootnoteContent f => new FootnoteContent(f.Label, text),
            _ => block.Content,
        };
    }

    // ================================================================
    // Serialization — produce JSON matching plugin block format
    // ================================================================

    private static string SerializeBlock(ShadowBlock block)
    {
        var json = block.Content switch
        {
            TextContent tc => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, text = tc.Text,
            }),
            HeadingContent hc => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, text = hc.Text, level = hc.Level,
            }),
            CodeContent cc => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, code = cc.Code, language = cc.Language,
            }),
            BlockquoteContent bq => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, text = bq.Text,
            }),
            CalloutContent co => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, text = co.Text, icon = co.Icon, variant = co.Variant,
            }),
            ListContent lc => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, items = SerializeListItems(lc.Items),
            }),
            TaskListContent tc => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, items = SerializeTaskListItems(tc.Items),
            }),
            ImageContent ic => SerializeImageBlock(block.Id, block.Type, ic),
            HorizontalRuleContent => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type,
            }),
            FootnoteContent fn => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, label = fn.Label, content = fn.Content, text = fn.Content,
            }),
            DefinitionListContent dl => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, items = dl.Items.Select(i => new
                {
                    id = i.Id, term = i.Term,
                    definitions = i.Definitions.Select(d => new { id = d.Id, text = d.Text }).ToArray(),
                }).ToArray(),
            }),
            TableContent tc => SerializeTableBlock(block.Id, block.Type, tc),
            TableOfContentsContent toc => JsonSerializer.Serialize(new
            {
                id = block.Id, type = block.Type, mode = toc.Mode,
            }),
            _ => JsonSerializer.Serialize(new { id = block.Id, type = block.Type }),
        };
        // Append layout fields if side-by-side
        if (block.Layout == "side_by_side" && block.PairId is not null)
        {
            // Re-serialize with layout — quick approach: parse and add fields
            using var doc = JsonDocument.Parse(json);
            using var ms = new System.IO.MemoryStream();
            using var writer = new Utf8JsonWriter(ms);
            writer.WriteStartObject();
            foreach (var prop in doc.RootElement.EnumerateObject())
                prop.WriteTo(writer);
            writer.WriteString("layout", "side_by_side");
            writer.WriteString("pair_id", block.PairId);
            writer.WriteString("pair_valign", block.PairValign);
            writer.WriteEndObject();
            writer.Flush();
            json = System.Text.Encoding.UTF8.GetString(ms.ToArray());
        }
        return json;
    }

    private static string SerializeTableBlock(string id, string type, TableContent tc)
    {
        using var ms = new System.IO.MemoryStream();
        using var writer = new Utf8JsonWriter(ms);
        writer.WriteStartObject();
        writer.WriteString("id", id);
        writer.WriteString("type", type);
        writer.WriteStartArray("columns");
        foreach (var col in tc.Columns) writer.WriteStringValue(col);
        writer.WriteEndArray();
        if (tc.HeaderRow is not null)
        {
            writer.WritePropertyName("header_row");
            WriteTableRow(writer, tc.HeaderRow);
        }
        writer.WriteStartArray("rows");
        foreach (var row in tc.Rows) WriteTableRow(writer, row);
        writer.WriteEndArray();
        writer.WriteBoolean("show_header", tc.ShowHeader);
        writer.WriteBoolean("alternating_rows", tc.AlternatingRows);
        if (tc.ColumnWidths is { Count: > 0 })
        {
            writer.WriteStartArray("column_widths");
            foreach (var w in tc.ColumnWidths) writer.WriteNumberValue(w);
            writer.WriteEndArray();
        }
        writer.WriteEndObject();
        writer.Flush();
        return System.Text.Encoding.UTF8.GetString(ms.ToArray());
    }

    private static string SerializeImageBlock(string id, string type, ImageContent ic)
    {
        using var ms = new System.IO.MemoryStream();
        using var writer = new Utf8JsonWriter(ms);
        writer.WriteStartObject();
        writer.WriteString("id", id);
        writer.WriteString("type", type);
        writer.WriteString("url", ic.Url);
        writer.WriteString("alt", ic.Alt ?? "");
        writer.WriteString("title", ic.Title ?? "");
        writer.WriteString("align", ic.Align);
        if (ic.Width.HasValue)
            writer.WriteNumber("width", ic.Width.Value);
        writer.WriteEndObject();
        writer.Flush();
        return System.Text.Encoding.UTF8.GetString(ms.ToArray());
    }

    private static void WriteTableRow(Utf8JsonWriter writer, ShadowTableRow row)
    {
        writer.WriteStartObject();
        writer.WriteString("id", row.Id);
        writer.WriteStartArray("cells");
        foreach (var cell in row.Cells)
        {
            writer.WriteStartObject();
            writer.WriteString("id", cell.Id);
            writer.WriteString("text", cell.Text);
            writer.WriteEndObject();
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }

    private static object[] SerializeListItems(List<ShadowListItem> items) =>
        items.Select(i => (object)new
        {
            id = i.Id,
            text = i.Text,
            children = SerializeListItems(i.Children),
        }).ToArray();

    private static object[] SerializeTaskListItems(List<ShadowListItem> items) =>
        items.Select(i => (object)new
        {
            id = i.Id,
            text = i.Text,
            is_checked = i.IsChecked,
            children = SerializeTaskListItems(i.Children),
        }).ToArray();

    // ================================================================
    // Markdown output helpers
    // ================================================================

    private static void AppendListItemsMd(StringBuilder sb, List<ShadowListItem> items, bool ordered, int depth)
    {
        var indent = new string(' ', depth * 2);
        var idx = 0;
        foreach (var item in items)
        {
            idx++;
            var prefix = ordered ? $"{idx}. " : "- ";
            sb.AppendLine($"{indent}{prefix}{item.Text}");
            if (item.Children.Count > 0)
                AppendListItemsMd(sb, item.Children, ordered, depth + 1);
        }
    }

    private static void AppendTaskItemsMd(StringBuilder sb, List<ShadowListItem> items, int depth)
    {
        var indent = new string(' ', depth * 2);
        foreach (var item in items)
        {
            var check = item.IsChecked ? "x" : " ";
            sb.AppendLine($"{indent}- [{check}] {item.Text}");
            if (item.Children.Count > 0)
                AppendTaskItemsMd(sb, item.Children, depth + 1);
        }
    }

    // ================================================================
    // Parse helpers for new block types
    // ================================================================

    private static List<ShadowDefinitionItem> ParseDefinitionItems(JsonElement block)
    {
        var result = new List<ShadowDefinitionItem>();
        if (block.TryGetProperty("items", out var items) && items.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in items.EnumerateArray())
            {
                var id = item.GetStringProp("id") ?? Guid.NewGuid().ToString("N")[..12];
                var term = item.GetStringProp("term") ?? "";
                var defs = new List<ShadowDefinitionEntry>();
                if (item.TryGetProperty("definitions", out var defsArr) && defsArr.ValueKind == JsonValueKind.Array)
                {
                    foreach (var d in defsArr.EnumerateArray())
                    {
                        defs.Add(new ShadowDefinitionEntry(
                            d.GetStringProp("id") ?? Guid.NewGuid().ToString("N")[..12],
                            d.GetStringProp("text") ?? ""));
                    }
                }
                if (defs.Count == 0) defs.Add(new ShadowDefinitionEntry(Guid.NewGuid().ToString("N")[..12], ""));
                result.Add(new ShadowDefinitionItem(id, term, defs));
            }
        }
        if (result.Count == 0)
            result.Add(new ShadowDefinitionItem(Guid.NewGuid().ToString("N")[..12], "", [new ShadowDefinitionEntry(Guid.NewGuid().ToString("N")[..12], "")]));
        return result;
    }

    private static List<string> ParseColumnAlignments(JsonElement block)
    {
        var result = new List<string>();
        if (block.TryGetProperty("columns", out var cols) && cols.ValueKind == JsonValueKind.Array)
        {
            foreach (var c in cols.EnumerateArray())
                result.Add(c.GetString() ?? "left");
        }
        if (result.Count == 0) result.AddRange(["left", "left", "left"]);
        return result;
    }

    private static List<double>? ParseColumnWidths(JsonElement block)
    {
        if (block.TryGetProperty("column_widths", out var cw) && cw.ValueKind == JsonValueKind.Array)
        {
            var result = new List<double>();
            foreach (var v in cw.EnumerateArray())
                result.Add(v.GetDouble());
            return result.Count > 0 ? result : null;
        }
        return null;
    }

    private static ShadowTableRow? ParseHeaderRow(JsonElement block)
    {
        if (!block.TryGetProperty("header_row", out var hr) || hr.ValueKind != JsonValueKind.Object) return null;
        return ParseSingleRow(hr);
    }

    private static List<ShadowTableRow> ParseTableRows(JsonElement block)
    {
        var result = new List<ShadowTableRow>();
        if (block.TryGetProperty("rows", out var rows) && rows.ValueKind == JsonValueKind.Array)
        {
            foreach (var r in rows.EnumerateArray())
            {
                var row = ParseSingleRow(r);
                if (row is not null) result.Add(row);
            }
        }
        return result;
    }

    private static ShadowTableRow? ParseSingleRow(JsonElement rowEl)
    {
        if (rowEl.ValueKind != JsonValueKind.Object) return null;
        var rowId = rowEl.GetStringProp("id") ?? Guid.NewGuid().ToString("N")[..12];
        var cells = new List<ShadowTableCell>();
        if (rowEl.TryGetProperty("cells", out var cellsArr) && cellsArr.ValueKind == JsonValueKind.Array)
        {
            foreach (var c in cellsArr.EnumerateArray())
            {
                cells.Add(new ShadowTableCell(
                    c.GetStringProp("id") ?? Guid.NewGuid().ToString("N")[..12],
                    c.GetStringProp("text") ?? ""));
            }
        }
        return new ShadowTableRow(rowId, cells);
    }

    // ================================================================
    // Public mutation helpers for new block types
    // ================================================================

    public void UpdateFootnoteLabel(string blockId, string label)
    {
        var block = GetBlock(blockId);
        if (block?.Content is FootnoteContent fn)
        {
            block.Content = new FootnoteContent(label, fn.Content);
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, label }));
        }
    }

    public void UpdateDefinitionTerm(string blockId, string itemId, string term)
    {
        var block = GetBlock(blockId);
        if (block?.Content is DefinitionListContent dl)
        {
            var items = dl.Items.Select(i =>
                i.Id == itemId ? i with { Term = term } : i).ToList();
            block.Content = new DefinitionListContent(items);
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, item_id = itemId, field = "term", value = term }));
        }
    }

    public void UpdateDefinitionText(string blockId, string itemId, string defId, string text)
    {
        var block = GetBlock(blockId);
        if (block?.Content is DefinitionListContent dl)
        {
            var items = dl.Items.Select(i =>
                i.Id == itemId
                    ? i with { Definitions = i.Definitions.Select(d => d.Id == defId ? d with { Text = text } : d).ToList() }
                    : i).ToList();
            block.Content = new DefinitionListContent(items);
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, item_id = itemId, def_id = defId, field = "text", value = text }));
        }
    }

    public void AddDefinitionItem(string blockId)
    {
        var block = GetBlock(blockId);
        if (block?.Content is DefinitionListContent dl)
        {
            var newItem = new ShadowDefinitionItem(
                Guid.NewGuid().ToString("N")[..12], "",
                [new ShadowDefinitionEntry(Guid.NewGuid().ToString("N")[..12], "")]);
            var items = dl.Items.ToList();
            items.Add(newItem);
            block.Content = new DefinitionListContent(items);
            Enqueue("add_definition_item", JsonSerializer.Serialize(new { id = blockId }));
        }
    }

    public void RemoveDefinitionItem(string blockId, string itemId)
    {
        var block = GetBlock(blockId);
        if (block?.Content is DefinitionListContent dl && dl.Items.Count > 1)
        {
            var items = dl.Items.Where(i => i.Id != itemId).ToList();
            block.Content = new DefinitionListContent(items);
            Enqueue("remove_definition_item", JsonSerializer.Serialize(new { id = blockId, item_id = itemId }));
        }
    }

    public void AddDefinition(string blockId, string itemId)
    {
        var block = GetBlock(blockId);
        if (block?.Content is DefinitionListContent dl)
        {
            var newDef = new ShadowDefinitionEntry(Guid.NewGuid().ToString("N")[..12], "");
            var items = dl.Items.Select(i =>
                i.Id == itemId ? i with { Definitions = i.Definitions.Append(newDef).ToList() } : i).ToList();
            block.Content = new DefinitionListContent(items);
            Enqueue("add_definition", JsonSerializer.Serialize(new { id = blockId, item_id = itemId }));
        }
    }

    public void RemoveDefinition(string blockId, string itemId, string defId)
    {
        var block = GetBlock(blockId);
        if (block?.Content is DefinitionListContent dl)
        {
            var items = dl.Items.Select(i =>
                i.Id == itemId && i.Definitions.Count > 1
                    ? i with { Definitions = i.Definitions.Where(d => d.Id != defId).ToList() }
                    : i).ToList();
            block.Content = new DefinitionListContent(items);
            Enqueue("remove_definition", JsonSerializer.Serialize(new { id = blockId, item_id = itemId, def_id = defId }));
        }
    }

    public void UpdateTableCell(string blockId, string rowId, string cellId, string text)
    {
        var block = GetBlock(blockId);
        if (block?.Content is TableContent tc)
        {
            ShadowTableRow? UpdateRow(ShadowTableRow? row) =>
                row is null ? null : row with { Cells = row.Cells.Select(c => c.Id == cellId ? c with { Text = text } : c).ToList() };

            block.Content = tc with
            {
                HeaderRow = rowId == tc.HeaderRow?.Id ? UpdateRow(tc.HeaderRow) : tc.HeaderRow,
                Rows = tc.Rows.Select(r => r.Id == rowId ? UpdateRow(r)! : r).ToList(),
            };
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, row_id = rowId, cell_id = cellId, cell_text = text }));
        }
    }

    public void ToggleTableHeader(string blockId)
    {
        var block = GetBlock(blockId);
        if (block?.Content is TableContent tc)
        {
            block.Content = tc with { ShowHeader = !tc.ShowHeader };
            Enqueue("toggle_header_row", JsonSerializer.Serialize(new { id = blockId }));
        }
    }

    public void ToggleTableAlternatingRows(string blockId)
    {
        var block = GetBlock(blockId);
        if (block?.Content is TableContent tc)
        {
            block.Content = tc with { AlternatingRows = !tc.AlternatingRows };
            Enqueue("toggle_alternating_rows", JsonSerializer.Serialize(new { id = blockId }));
        }
    }

    public void SortTableColumn(string blockId, int colIndex, string direction)
    {
        var block = GetBlock(blockId);
        if (block?.Content is TableContent tc)
        {
            var sorted = tc.Rows.OrderBy(r =>
            {
                var text = colIndex < r.Cells.Count ? r.Cells[colIndex].Text : "";
                return (object)text;
            }, new TableCellComparer()).ToList();
            if (direction == "desc") sorted.Reverse();
            block.Content = tc with { Rows = sorted };
            Enqueue("sort_table_column", JsonSerializer.Serialize(new { id = blockId, col_index = colIndex, direction }));
        }
    }

    public void SetColumnWidths(string blockId, List<double> widths)
    {
        var block = GetBlock(blockId);
        if (block?.Content is TableContent tc)
        {
            block.Content = tc with { ColumnWidths = widths };
            Enqueue("set_column_widths", JsonSerializer.Serialize(new { id = blockId, widths }));
        }
    }

    private sealed class TableCellComparer : IComparer<object>
    {
        public int Compare(object? x, object? y)
        {
            var sx = x?.ToString() ?? "";
            var sy = y?.ToString() ?? "";
            if (double.TryParse(sx, out var nx) && double.TryParse(sy, out var ny))
                return nx.CompareTo(ny);
            return string.Compare(sx, sy, StringComparison.OrdinalIgnoreCase);
        }
    }

    public void UpdatePairValign(string pairId, string valign)
    {
        foreach (var block in _blocks)
        {
            if (block.PairId == pairId && block.Layout == "side_by_side")
                block.PairValign = valign;
        }
        Enqueue("set_pair_valign", JsonSerializer.Serialize(new { pair_id = pairId, valign }));
    }

    public void UnpairBlocks(string pairId)
    {
        foreach (var block in _blocks)
        {
            if (block.PairId == pairId && block.Layout == "side_by_side")
            {
                block.Layout = "single";
                block.PairId = null;
                block.PairValign = "top";
            }
        }
        Enqueue("unpair_blocks", JsonSerializer.Serialize(new { pair_id = pairId }));
    }

    public void PairBlocks(string blockIdA, string blockIdB)
    {
        var pairId = Guid.NewGuid().ToString("N")[..12];
        // Ensure adjacency in shadow: move B right after A
        var idxA = _blocks.FindIndex(b => b.Id == blockIdA);
        var idxB = _blocks.FindIndex(b => b.Id == blockIdB);
        if (idxA >= 0 && idxB >= 0 && idxB != idxA + 1)
        {
            var blockB = _blocks[idxB];
            _blocks.RemoveAt(idxB);
            var newIdxA = _blocks.FindIndex(b => b.Id == blockIdA);
            _blocks.Insert(newIdxA + 1, blockB);
        }
        foreach (var block in _blocks)
        {
            if (block.Id == blockIdA || block.Id == blockIdB)
            {
                block.Layout = "side_by_side";
                block.PairId = pairId;
                block.PairValign = "top";
            }
        }
        Enqueue("pair_blocks", JsonSerializer.Serialize(new { block_id_a = blockIdA, block_id_b = blockIdB }));
    }

    public void UpdateImageUrl(string blockId, string url)
    {
        var block = GetBlock(blockId);
        if (block?.Content is ImageContent ic)
        {
            block.Content = ic with { Url = url };
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, url }));
        }
    }

    public void UpdateImageAlt(string blockId, string alt)
    {
        var block = GetBlock(blockId);
        if (block?.Content is ImageContent ic)
        {
            block.Content = ic with { Alt = alt };
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, alt }));
        }
    }

    public void UpdateImageTitle(string blockId, string title)
    {
        var block = GetBlock(blockId);
        if (block?.Content is ImageContent ic)
        {
            block.Content = ic with { Title = title };
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, title }));
        }
    }

    public void UpdateImageAlign(string blockId, string align)
    {
        var block = GetBlock(blockId);
        if (block?.Content is ImageContent ic)
        {
            block.Content = ic with { Align = align };
            Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, align }));
        }
    }

    public void UpdateImageWidth(string blockId, double? width)
    {
        var block = GetBlock(blockId);
        if (block?.Content is ImageContent ic)
        {
            block.Content = ic with { Width = width };
            if (width.HasValue)
                Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, width = width.Value }));
            else
                Enqueue("update_block", JsonSerializer.Serialize(new { id = blockId, width = (double?)null }));
        }
    }
}
