using Avalonia.Controls;
using PrivStack.Desktop.Plugins.Graph.ViewModels;
using PrivStack.UI.Adaptive.Controls;
using PrivStack.UI.Adaptive.Services;

namespace PrivStack.Desktop.Plugins.Graph.Views;

public partial class GraphView : UserControl
{
    private NeuronGraphControl? _graphControl;
    private EmbeddingSpaceControl? _embeddingControl;
    private GraphViewModel? _vm;

    public GraphView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object? sender, EventArgs e)
    {
        if (_vm != null)
        {
            _vm.PropertyChanged -= OnVmPropertyChanged;
            _vm.RequestReheat -= OnRequestReheat;
            _vm.RequestResetView -= OnRequestResetView;
            _vm.PhysicsParametersChanged -= OnPhysicsChanged;
            _vm.ActiveTabChanged -= OnActiveTabChanged;
            if (_vm.EmbeddingSpace != null)
                _vm.EmbeddingSpace.AutoRotateChanged -= OnAutoRotateChanged;
        }

        if (DataContext is not GraphViewModel vm) return;
        _vm = vm;

        _vm.PropertyChanged += OnVmPropertyChanged;
        _vm.RequestReheat += OnRequestReheat;
        _vm.RequestResetView += OnRequestResetView;
        _vm.PhysicsParametersChanged += OnPhysicsChanged;
        _vm.ActiveTabChanged += OnActiveTabChanged;
        if (_vm.EmbeddingSpace != null)
            _vm.EmbeddingSpace.AutoRotateChanged += OnAutoRotateChanged;
    }

    private void OnVmPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (_vm == null) return;

        if (e.PropertyName == nameof(GraphViewModel.GraphData))
            UpdateGraphCanvas();

        if (e.PropertyName == nameof(GraphViewModel.HighlightDepth) && _graphControl != null)
            _graphControl.HighlightDepth = _vm.HighlightDepth;

        if (e.PropertyName == nameof(GraphViewModel.HideInactiveNodes) && _graphControl != null)
            _graphControl.HideInactiveNodes = _vm.HideInactiveNodes;

        // Update embedding control when embedding data changes
        if (e.PropertyName is "EmbeddingSpace" && _vm.EmbeddingSpace != null)
        {
            _vm.EmbeddingSpace.PropertyChanged += OnEmbeddingPropertyChanged;
        }
    }

    private void OnEmbeddingPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(EmbeddingSpaceViewModel.EmbeddingData))
            UpdateEmbeddingCanvas();

        if (e.PropertyName == nameof(EmbeddingSpaceViewModel.SelectedIndex))
        {
            var selInfo = this.FindControl<StackPanel>("EmbeddingSelectionInfo");
            if (selInfo != null)
                selInfo.IsVisible = (_vm?.EmbeddingSpace?.SelectedIndex ?? -1) >= 0;
        }
    }

    // ========================================================================
    // Tab switching
    // ========================================================================

    private void OnActiveTabChanged(object? sender, GraphTab tab)
    {
        var host = this.FindControl<Border>("GraphCanvasHost");
        var kgSidebar = this.FindControl<StackPanel>("KnowledgeGraphSidebar");
        var embSidebar = this.FindControl<StackPanel>("EmbeddingSpaceSidebar");
        var kgFooter = this.FindControl<StackPanel>("KnowledgeGraphFooter");
        var embFooter = this.FindControl<StackPanel>("EmbeddingSpaceFooter");
        var tabKg = this.FindControl<Button>("TabKnowledgeGraph");
        var tabEmb = this.FindControl<Button>("TabEmbeddingSpace");

        var isKg = tab == GraphTab.KnowledgeGraph;

        if (kgSidebar != null) kgSidebar.IsVisible = isKg;
        if (embSidebar != null) embSidebar.IsVisible = !isKg;
        if (kgFooter != null) kgFooter.IsVisible = isKg;
        if (embFooter != null) embFooter.IsVisible = !isKg;

        // Update tab button styling
        if (tabKg != null) tabKg.FontWeight = isKg ? Avalonia.Media.FontWeight.Bold : Avalonia.Media.FontWeight.Normal;
        if (tabEmb != null) tabEmb.FontWeight = !isKg ? Avalonia.Media.FontWeight.Bold : Avalonia.Media.FontWeight.Normal;

        if (host == null) return;

        if (isKg)
        {
            // Switch to knowledge graph
            _embeddingControl?.ClearData();
            host.Child = null;
            _embeddingControl = null;
            UpdateGraphCanvas();
        }
        else
        {
            // Switch to embedding space
            host.Child = null;
            _graphControl = null;
            EnsureEmbeddingControl(host);

            // Subscribe to embedding VM property changes
            if (_vm?.EmbeddingSpace != null)
            {
                _vm.EmbeddingSpace.PropertyChanged -= OnEmbeddingPropertyChanged;
                _vm.EmbeddingSpace.PropertyChanged += OnEmbeddingPropertyChanged;
                _ = _vm.EmbeddingSpace.LoadAsync();
            }
        }
    }

    // ========================================================================
    // Knowledge Graph
    // ========================================================================

    private void UpdateGraphCanvas()
    {
        if (_vm?.ActiveTab != GraphTab.KnowledgeGraph) return;

        if (_vm.GraphData == null || _vm.GraphData.NodeCount == 0)
        {
            var host = this.FindControl<Border>("GraphCanvasHost");
            if (host != null) host.Child = null;
            _graphControl = null;

            var emptyText = this.FindControl<TextBlock>("EmptyStateText");
            if (emptyText != null) emptyText.IsVisible = !(_vm.IsLoading);
            return;
        }

        var emptyState = this.FindControl<TextBlock>("EmptyStateText");
        if (emptyState != null) emptyState.IsVisible = false;

        _vm.GraphData.AssignBfsDepths(_vm.CenterNodeId);

        EnsureGraphControl();

        if (_graphControl == null) return;

        _graphControl.CenterId = _vm.CenterNodeId;
        _graphControl.HighlightDepth = _vm.HighlightDepth;
        _graphControl.HideInactiveNodes = _vm.HideInactiveNodes;
        _graphControl.Physics = new PhysicsParameters
        {
            RepelRadius = _vm.RepelRadius,
            CenterStrength = _vm.CenterForce,
            LinkDistance = _vm.LinkDistance,
            LinkStrength = _vm.LinkForce,
        };
        _graphControl.StartWithData(_vm.GraphData);
    }

    private void EnsureGraphControl()
    {
        if (_graphControl != null) return;

        var host = this.FindControl<Border>("GraphCanvasHost");
        if (host == null) return;

        _graphControl = new NeuronGraphControl();
        _graphControl.EnableHighlightMode = true;
        _graphControl.NodeClicked += OnNodeClicked;
        _graphControl.NodeDeselected += OnNodeDeselected;
        host.Child = _graphControl;
    }

    // ========================================================================
    // Embedding Space
    // ========================================================================

    private void UpdateEmbeddingCanvas()
    {
        if (_vm?.ActiveTab != GraphTab.EmbeddingSpace) return;
        if (_embeddingControl == null) return;

        var data = _vm.EmbeddingSpace?.EmbeddingData;
        if (data == null || data.Points.Count == 0)
        {
            _embeddingControl.ClearData();
            return;
        }

        _embeddingControl.SetData(data);
    }

    private void EnsureEmbeddingControl(Border host)
    {
        if (_embeddingControl != null) return;

        _embeddingControl = new EmbeddingSpaceControl();
        _embeddingControl.Focusable = true;
        _embeddingControl.PointClicked += OnEmbeddingPointClicked;
        _embeddingControl.PointDeselected += OnEmbeddingPointDeselected;
        host.Child = _embeddingControl;

        // Sync auto-rotate state
        if (_vm?.EmbeddingSpace != null)
            _embeddingControl.Camera.IsAutoRotating = _vm.EmbeddingSpace.AutoRotate;
    }

    private void OnEmbeddingPointClicked(int index) => _vm?.EmbeddingSpace?.OnPointClicked(index);
    private void OnEmbeddingPointDeselected() => _vm?.EmbeddingSpace?.OnPointDeselected();
    private void OnAutoRotateChanged(object? sender, bool value)
    {
        if (_embeddingControl != null)
            _embeddingControl.Camera.IsAutoRotating = value;
    }

    // ========================================================================
    // Common event handlers
    // ========================================================================

    private void OnNodeClicked(string nodeId) => _vm?.OnNodeClicked(nodeId);
    private void OnNodeDeselected() => _vm?.OnNodeDeselected();

    private void OnRequestReheat(object? sender, EventArgs e) => UpdateGraphCanvas();

    private void OnRequestResetView(object? sender, EventArgs e)
    {
        var host = this.FindControl<Border>("GraphCanvasHost");
        if (host != null) host.Child = null;
        _graphControl = null;
    }

    private void OnPhysicsChanged(object? sender, EventArgs e)
    {
        if (_graphControl == null || _vm == null) return;
        _graphControl.Physics = new PhysicsParameters
        {
            RepelRadius = _vm.RepelRadius,
            CenterStrength = _vm.CenterForce,
            LinkDistance = _vm.LinkDistance,
            LinkStrength = _vm.LinkForce,
        };
        _graphControl.ApplyPhysicsChanges();
    }
}
