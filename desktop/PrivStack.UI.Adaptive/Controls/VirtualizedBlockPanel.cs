// VirtualizedBlockPanel.cs
// Lazy block rendering for efficient large document handling.
// Creates lightweight placeholders initially, renders actual controls on-demand as they scroll into view.

using System;
using System.Collections.Generic;
using System.Text.Json;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Threading;

namespace PrivStack.UI.Adaptive.Controls;

/// <summary>
/// Metadata for a block used for virtualization.
/// </summary>
public sealed class BlockMetadata
{
    public required string Id { get; init; }
    public required string Type { get; init; }
    public required JsonElement Data { get; init; }

    /// <summary>Estimated height in pixels (before actual render).</summary>
    public double EstimatedHeight { get; set; }

    /// <summary>Actual measured height after render (0 if not yet rendered).</summary>
    public double MeasuredHeight { get; set; }

    /// <summary>Whether this block has been rendered.</summary>
    public bool IsRendered { get; set; }

    /// <summary>Index in the parent panel's Children collection.</summary>
    public int ChildIndex { get; set; }

    /// <summary>For side-by-side blocks: the pair ID.</summary>
    public string? PairId { get; init; }

    /// <summary>Layout mode (e.g., "side_by_side").</summary>
    public string? Layout { get; init; }
}

/// <summary>
/// Manages lazy/virtualized rendering of blocks within a StackPanel.
/// Renders blocks on-demand as they scroll into view.
/// </summary>
public sealed class LazyBlockRenderer : IDisposable
{
    // Configuration
    private const int BufferBlocks = 8; // Extra blocks to render above/below viewport
    private const int MinBlocksForVirtualization = 30; // Don't virtualize small documents
    private const double DefaultBlockHeight = 40.0;

    private readonly List<BlockMetadata> _blocks = new();
    private readonly Dictionary<string, Control> _renderedControls = new();
    private readonly StackPanel _panel;
    private ScrollViewer? _scrollViewer;

    private Func<BlockMetadata, Control>? _renderBlock;
    private Action<Control, string>? _wireEvents;
    private bool _useVirtualization;

    public LazyBlockRenderer(StackPanel panel, ScrollViewer? scrollViewer = null)
    {
        _panel = panel;
        _scrollViewer = scrollViewer;

        if (_scrollViewer != null)
        {
            _scrollViewer.ScrollChanged += OnScrollChanged;
        }
        else
        {
            // Panel not yet in visual tree - hook into AttachedToVisualTree to find ScrollViewer later
            _panel.AttachedToVisualTree += OnPanelAttached;
        }
    }

    private void OnPanelAttached(object? sender, Avalonia.VisualTreeAttachmentEventArgs e)
    {
        _panel.AttachedToVisualTree -= OnPanelAttached;

        if (_scrollViewer == null)
        {
            _scrollViewer = FindParentScrollViewer(_panel);
            if (_scrollViewer != null)
            {
                _scrollViewer.ScrollChanged += OnScrollChanged;
                // Render visible blocks now that we can calculate viewport
                Dispatcher.UIThread.Post(RenderVisibleBlocks, DispatcherPriority.Loaded);
            }
            else
            {
                // No scroll viewer found - render all blocks as fallback
                RenderAllBlocksFallback();
            }
        }
    }

    private void RenderAllBlocksFallback()
    {
        if (_renderBlock == null) return;

        for (var i = 0; i < _blocks.Count; i++)
        {
            var block = _blocks[i];
            if (!block.IsRendered)
            {
                RenderBlockAt(block);
            }
        }
    }

    /// <summary>
    /// Initializes with block data.
    /// </summary>
    /// <param name="blocksJson">JSON array of blocks.</param>
    /// <param name="renderBlock">Function to render a block metadata into a Control.</param>
    /// <param name="wireEvents">Optional: function to wire events to a rendered control.</param>
    public void Initialize(
        JsonElement blocksJson,
        Func<BlockMetadata, Control> renderBlock,
        Action<Control, string>? wireEvents = null)
    {
        _renderBlock = renderBlock;
        _wireEvents = wireEvents;
        _blocks.Clear();
        _renderedControls.Clear();

        if (blocksJson.ValueKind != JsonValueKind.Array)
        {
                _useVirtualization = false;
            return;
        }

        var blockCount = blocksJson.GetArrayLength();
        _useVirtualization = blockCount >= MinBlocksForVirtualization;

        // Parse all blocks
        var index = 0;
        foreach (var block in blocksJson.EnumerateArray())
        {
            var id = GetStringProp(block, "id") ?? Guid.NewGuid().ToString();
            var type = GetStringProp(block, "type") ?? "paragraph";

            var metadata = new BlockMetadata
            {
                Id = id,
                Type = type,
                Data = block.Clone(),
                PairId = GetStringProp(block, "pair_id"),
                Layout = GetStringProp(block, "layout"),
                EstimatedHeight = EstimateBlockHeight(block, type),
                ChildIndex = index,
            };

            _blocks.Add(metadata);
            index++;
        }


        if (!_useVirtualization)
        {
            // Small document - render everything immediately (existing behavior)
            RenderAllBlocks();
        }
        else
        {
            // Large document - create placeholders, render visible on-demand
            CreatePlaceholders();

            // Only schedule render if we have a scroll viewer; otherwise wait for AttachedToVisualTree
            if (_scrollViewer != null)
            {
                Dispatcher.UIThread.Post(RenderVisibleBlocks, DispatcherPriority.Loaded);
            }
            // If no scroll viewer, OnPanelAttached will trigger rendering
        }
    }

    /// <summary>
    /// Call this when blocks are added/removed to update tracking.
    /// </summary>
    public void RefreshBlockIndices()
    {
        for (var i = 0; i < _blocks.Count; i++)
        {
            _blocks[i].ChildIndex = i;
        }
    }

    /// <summary>
    /// Manually triggers rendering of visible blocks (call after panel resize, etc.)
    /// </summary>
    public void UpdateVisibility()
    {
        if (_useVirtualization)
            RenderVisibleBlocks();
    }

    /// <summary>
    /// Gets whether a specific block has been rendered.
    /// </summary>
    public bool IsBlockRendered(string blockId) => _renderedControls.ContainsKey(blockId);

    /// <summary>
    /// Gets the rendered control for a block, or null if not yet rendered.
    /// </summary>
    public Control? GetRenderedControl(string blockId) =>
        _renderedControls.GetValueOrDefault(blockId);

    /// <summary>
    /// Force-renders a specific block (e.g., for focus/scroll-to operations).
    /// </summary>
    public Control? EnsureBlockRendered(string blockId)
    {
        var block = _blocks.Find(b => b.Id == blockId);
        if (block == null) return null;

        if (!block.IsRendered)
        {
            RenderBlockAt(block);
        }

        return _renderedControls.GetValueOrDefault(blockId);
    }

    /// <summary>
    /// Gets block count.
    /// </summary>
    public int BlockCount => _blocks.Count;

    /// <summary>
    /// Gets all block metadata.
    /// </summary>
    public IReadOnlyList<BlockMetadata> Blocks => _blocks;

    public void Dispose()
    {
        if (_scrollViewer != null)
        {
            _scrollViewer.ScrollChanged -= OnScrollChanged;
        }
    }

    private void OnScrollChanged(object? sender, ScrollChangedEventArgs e)
    {
        if (_useVirtualization)
            RenderVisibleBlocks();
    }

    private void CreatePlaceholders()
    {
        _panel.Children.Clear();

        foreach (var block in _blocks)
        {
            var placeholder = new Border
            {
                Height = block.EstimatedHeight,
                Tag = $"placeholder:{block.Id}",
                Background = null,
                // Add subtle visual feedback that content is loading
                // Uncomment for debug: Background = new Avalonia.Media.SolidColorBrush(Avalonia.Media.Color.FromArgb(20, 128, 128, 128)),
            };
            _panel.Children.Add(placeholder);
        }
    }

    private void RenderAllBlocks()
    {
        _panel.Children.Clear();

        foreach (var block in _blocks)
        {
            RenderBlockAt(block);
        }
    }

    private void RenderVisibleBlocks()
    {
        if (_scrollViewer == null || _renderBlock == null || _blocks.Count == 0)
            return;

        var scrollOffset = _scrollViewer.Offset.Y;
        var viewportHeight = _scrollViewer.Viewport.Height;

        // Find visible range
        var (firstVisible, lastVisible) = CalculateVisibleRange(scrollOffset, viewportHeight);

        // Add buffer
        firstVisible = Math.Max(0, firstVisible - BufferBlocks);
        lastVisible = Math.Min(_blocks.Count - 1, lastVisible + BufferBlocks);

        // Render blocks in visible range that haven't been rendered yet
        for (var i = firstVisible; i <= lastVisible; i++)
        {
            var block = _blocks[i];
            if (!block.IsRendered)
            {
                RenderBlockAt(block);
            }
        }
    }

    private (int First, int Last) CalculateVisibleRange(double scrollOffset, double viewportHeight)
    {
        var currentOffset = 0.0;
        var spacing = _panel.Spacing;
        var firstVisible = -1;
        var lastVisible = -1;

        for (var i = 0; i < _blocks.Count; i++)
        {
            var block = _blocks[i];
            var height = block.IsRendered ? block.MeasuredHeight : block.EstimatedHeight;
            if (height <= 0) height = DefaultBlockHeight;

            var blockTop = currentOffset;
            var blockBottom = currentOffset + height;

            // Check if block intersects viewport
            if (blockBottom >= scrollOffset && blockTop <= scrollOffset + viewportHeight)
            {
                if (firstVisible < 0) firstVisible = i;
                lastVisible = i;
            }
            else if (firstVisible >= 0 && blockTop > scrollOffset + viewportHeight)
            {
                // Past viewport, stop searching
                break;
            }

            currentOffset = blockBottom + spacing;
        }

        if (firstVisible < 0) firstVisible = 0;
        if (lastVisible < 0) lastVisible = 0;

        return (firstVisible, lastVisible);
    }

    private void RenderBlockAt(BlockMetadata block)
    {
        if (_renderBlock == null || block.IsRendered)
            return;

        try
        {
            var control = _renderBlock(block);

            // Replace placeholder with actual control
            if (block.ChildIndex < _panel.Children.Count)
            {
                _panel.Children[block.ChildIndex] = control;
            }

            _renderedControls[block.Id] = control;
            block.IsRendered = true;

            // Wire events if provided
            _wireEvents?.Invoke(control, block.Id);

            // Measure actual height
            control.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
            var measuredHeight = control.DesiredSize.Height;
            if (measuredHeight > 0)
            {
                block.MeasuredHeight = measuredHeight;
            }
        }
        catch (Exception)
        {
            // On error, leave placeholder in place
            block.IsRendered = false;
        }
    }

    private static ScrollViewer? FindParentScrollViewer(Control control)
    {
        var parent = control.Parent;
        while (parent != null)
        {
            if (parent is ScrollViewer sv)
                return sv;
            parent = parent.Parent;
        }
        return null;
    }

    private static double EstimateBlockHeight(JsonElement block, string type)
    {
        return type switch
        {
            "paragraph" => EstimateParagraphHeight(block),
            "heading" => EstimateHeadingHeight(block),
            "code_block" => EstimateCodeBlockHeight(block),
            "blockquote" => EstimateParagraphHeight(block) * 1.2,
            "callout" => EstimateParagraphHeight(block) * 1.3,
            "bullet_list" or "numbered_list" => EstimateListHeight(block),
            "task_list" => EstimateListHeight(block),
            "image" => 200.0,
            "table" => EstimateTableHeight(block),
            "table_of_contents" => 100.0,
            "footnote" => DefaultBlockHeight,
            "definition_list" => EstimateListHeight(block) * 1.5,
            _ => DefaultBlockHeight,
        };
    }

    private static double EstimateParagraphHeight(JsonElement block)
    {
        var text = GetStringProp(block, "text") ?? "";
        var charCount = text.Length;
        var estimatedLines = Math.Max(1, (int)Math.Ceiling(charCount / 80.0));
        return estimatedLines * 24.0 + 8.0;
    }

    private static double EstimateHeadingHeight(JsonElement block)
    {
        var level = 1;
        if (block.TryGetProperty("level", out var levelProp) && levelProp.ValueKind == JsonValueKind.Number)
            level = levelProp.GetInt32();

        var baseHeight = level switch
        {
            1 => 48.0,
            2 => 36.0,
            3 => 28.0,
            _ => 24.0,
        };

        return baseHeight + 16.0;
    }

    private static double EstimateCodeBlockHeight(JsonElement block)
    {
        var code = GetStringProp(block, "code") ?? GetStringProp(block, "text") ?? "";
        var lineCount = code.Split('\n').Length;
        return Math.Max(60.0, lineCount * 20.0 + 40.0);
    }

    private static double EstimateListHeight(JsonElement block)
    {
        var items = 3;
        if (block.TryGetProperty("items", out var itemsArray) && itemsArray.ValueKind == JsonValueKind.Array)
        {
            items = itemsArray.GetArrayLength();
        }
        return items * 28.0 + 16.0;
    }

    private static double EstimateTableHeight(JsonElement block)
    {
        var rows = 3;
        if (block.TryGetProperty("cells", out var cells) && cells.ValueKind == JsonValueKind.Array)
        {
            rows = cells.GetArrayLength();
        }
        return rows * 32.0 + 40.0;
    }

    private static string? GetStringProp(JsonElement el, string name)
    {
        if (el.TryGetProperty(name, out var prop) && prop.ValueKind == JsonValueKind.String)
            return prop.GetString();
        return null;
    }
}
