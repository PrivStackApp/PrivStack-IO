using System;
using System.Collections.Concurrent;
using System.Diagnostics.CodeAnalysis;
using Avalonia.Controls;
using Avalonia.Controls.Templates;
using PrivStack.Desktop.ViewModels;

namespace PrivStack.Desktop;

/// <summary>
/// Given a view model, returns the corresponding view if possible.
/// Convention: "FooViewModel" → "FooView" (same namespace/assembly).
/// No adapter unwrapping needed — plugins return Sdk.ViewModelBase directly.
/// </summary>
[RequiresUnreferencedCode(
    "Default implementation of ViewLocator involves reflection which may be trimmed away.",
    Url = "https://docs.avaloniaui.net/docs/concepts/view-locator")]
public class ViewLocator : IDataTemplate
{
    private static readonly ConcurrentDictionary<string, Type?> _typeCache = new();

    public Control? Build(object? param)
    {
        if (param is null)
            return null;

        // Special case: Wasm plugin proxy → generic renderer
        if (param is PrivStack.Desktop.Services.Plugin.WasmViewModelProxy)
        {
            return new Views.WasmPluginView();
        }

        var vmType = param.GetType();
        var viewName = vmType.FullName!.Replace("ViewModel", "View", StringComparison.Ordinal);

        var viewType = _typeCache.GetOrAdd(viewName, name =>
        {
            // 1. Try the ViewModel's own assembly first (handles external plugins)
            var type = vmType.Assembly.GetType(name);
            if (type != null) return type;

            // 2. Try the Desktop (host) assembly
            type = typeof(ViewLocator).Assembly.GetType(name);
            if (type != null) return type;

            // 3. Fallback: search all loaded assemblies (slow, cached)
            foreach (var assembly in AppDomain.CurrentDomain.GetAssemblies())
            {
                try
                {
                    type = assembly.GetType(name);
                    if (type != null) return type;
                }
                catch
                {
                    // Skip assemblies that throw on GetType
                }
            }

            return null;
        });

        if (viewType != null)
        {
            return (Control)Activator.CreateInstance(viewType)!;
        }

        return new TextBlock { Text = "Not Found: " + viewName };
    }

    public bool Match(object? data)
    {
        return data is ViewModelBase || data is PrivStack.Sdk.ViewModelBase;
    }
}
