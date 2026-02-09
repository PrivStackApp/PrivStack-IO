using System.Collections.ObjectModel;
using System.Linq;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using PrivStack.Desktop.Models;
using PrivStack.Desktop.Native;
using PrivStack.Desktop.Services;
using PrivStack.Desktop.Services.Abstractions;

namespace PrivStack.Desktop.ViewModels;

/// <summary>
/// ViewModel for the sync pairing flow.
/// Handles sync code generation/entry, peer discovery, approval, and trusted peer management.
/// </summary>
public partial class SyncPairingViewModel : ViewModelBase
{
    private readonly ISyncService _syncService;
    private readonly IPairingService _pairingService;
    private readonly IUiDispatcher _dispatcher;
    private readonly IAppSettingsService _appSettings;
    private System.Timers.Timer? _discoveryTimer;
    private int _discoveryRunning; // re-entrancy guard

    // ========================================================================
    // Observable Properties
    // ========================================================================

    /// <summary>
    /// Current step in the pairing flow.
    /// </summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsCodeStep))]
    [NotifyPropertyChangedFor(nameof(IsDiscoveryStep))]
    [NotifyPropertyChangedFor(nameof(IsTrustedStep))]
    private PairingStep _currentStep = PairingStep.EnterCode;

    /// <summary>
    /// The current sync code (if set).
    /// </summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasSyncCode))]
    [NotifyPropertyChangedFor(nameof(SyncCodeDisplay))]
    private SyncCode? _syncCode;

    /// <summary>
    /// User input for entering a sync code.
    /// </summary>
    [ObservableProperty]
    private string _syncCodeInput = string.Empty;

    /// <summary>
    /// Error message to display.
    /// </summary>
    [ObservableProperty]
    private string? _errorMessage;

    /// <summary>
    /// Status message to display.
    /// </summary>
    [ObservableProperty]
    private string _statusMessage = "Enter a sync code or generate a new one";

    /// <summary>
    /// Whether we're currently discovering peers.
    /// </summary>
    [ObservableProperty]
    private bool _isDiscovering;

    /// <summary>
    /// The device name for this device.
    /// </summary>
    [ObservableProperty]
    private string _deviceName = string.Empty;

    /// <summary>
    /// Discovered peers pending approval.
    /// </summary>
    public ObservableCollection<PairingPeerInfo> DiscoveredPeers { get; } = [];

    /// <summary>
    /// Trusted peers.
    /// </summary>
    public ObservableCollection<TrustedPeer> TrustedPeers { get; } = [];

    // ========================================================================
    // Computed Properties
    // ========================================================================

    public bool IsCodeStep => CurrentStep == PairingStep.EnterCode;
    public bool IsDiscoveryStep => CurrentStep == PairingStep.Discovery;
    public bool IsTrustedStep => CurrentStep == PairingStep.TrustedPeers;
    public bool HasSyncCode => SyncCode != null;
    public string SyncCodeDisplay => SyncCode?.Code ?? "No code set";

    // ========================================================================
    // Constructor
    // ========================================================================

    public SyncPairingViewModel(ISyncService syncService, IPairingService pairingService, IUiDispatcher dispatcher, IAppSettingsService appSettings)
    {
        _syncService = syncService;
        _pairingService = pairingService;
        _dispatcher = dispatcher;
        _appSettings = appSettings;
        LoadInitialState();
    }

    // ========================================================================
    // Commands
    // ========================================================================

    /// <summary>
    /// Generates a new random sync code.
    /// </summary>
    [RelayCommand]
    private async Task GenerateCode()
    {
        try
        {
            ErrorMessage = null;
            SyncCode = await Task.Run(() => _pairingService.GenerateSyncCode());
            StatusMessage = "Share this code with your other device";
            Log.Information("Generated sync code: {Code}", SyncCode.Code);
            await PersistPairingStateAsync();
        }
        catch (Exception ex)
        {
            ErrorMessage = $"Failed to generate code: {ex.Message}";
            Log.Error(ex, "Failed to generate sync code");
        }
    }

    /// <summary>
    /// Sets the sync code from user input.
    /// </summary>
    [RelayCommand]
    private async Task JoinWithCode()
    {
        if (string.IsNullOrWhiteSpace(SyncCodeInput))
        {
            ErrorMessage = "Please enter a sync code";
            return;
        }

        try
        {
            ErrorMessage = null;
            var input = SyncCodeInput;
            await Task.Run(() => _pairingService.SetSyncCode(input));
            SyncCode = await Task.Run(() => _pairingService.GetSyncCode());
            StatusMessage = "Sync code set. Starting discovery...";
            SyncCodeInput = string.Empty;
            Log.Information("Set sync code: {Code}", SyncCode?.Code);
            await PersistPairingStateAsync();

            // Move to discovery step
            await StartDiscovery();
        }
        catch (Exception ex)
        {
            ErrorMessage = $"Invalid sync code: {ex.Message}";
            Log.Error(ex, "Failed to set sync code");
        }
    }

    /// <summary>
    /// Starts peer discovery with the current sync code.
    /// </summary>
    [RelayCommand]
    private async Task StartDiscovery()
    {
        if (SyncCode == null)
        {
            ErrorMessage = "No sync code set";
            return;
        }

        try
        {
            // Start sync if not already running — this is the heavy call
            var isRunning = await Task.Run(() => _syncService.IsSyncRunning());
            if (!isRunning)
            {
                StatusMessage = "Starting P2P transport...";
                await Task.Run(() => _syncService.StartSync());
            }

            CurrentStep = PairingStep.Discovery;
            IsDiscovering = true;
            StatusMessage = "Searching for devices with the same code...";

            // Start discovery timer
            StartDiscoveryTimer();

            Log.Information("Started peer discovery");
        }
        catch (Exception ex)
        {
            ErrorMessage = $"Failed to start discovery: {ex.Message}";
            Log.Error(ex, "Failed to start discovery");
        }
    }

    /// <summary>
    /// Stops peer discovery.
    /// </summary>
    [RelayCommand]
    private void StopDiscovery()
    {
        StopDiscoveryTimer();
        IsDiscovering = false;
        StatusMessage = "Discovery stopped";
        Log.Information("Stopped peer discovery");
    }

    /// <summary>
    /// Approves a discovered peer, making them trusted.
    /// </summary>
    [RelayCommand]
    private async Task ApprovePeer(PairingPeerInfo peer)
    {
        try
        {
            await Task.Run(() => _pairingService.ApprovePeer(peer.PeerId));
            DiscoveredPeers.Remove(peer);
            var trustedPeers = await Task.Run(() => _pairingService.GetTrustedPeers());
            TrustedPeers.Clear();
            foreach (var p in trustedPeers)
                TrustedPeers.Add(p);
            StatusMessage = $"Approved {peer.DeviceName}";
            Log.Information("Approved peer: {PeerId} ({DeviceName})", peer.PeerId, peer.DeviceName);
            await PersistPairingStateAsync();
        }
        catch (Exception ex)
        {
            ErrorMessage = $"Failed to approve peer: {ex.Message}";
            Log.Error(ex, "Failed to approve peer");
        }
    }

    /// <summary>
    /// Rejects a discovered peer.
    /// </summary>
    [RelayCommand]
    private async Task RejectPeer(PairingPeerInfo peer)
    {
        try
        {
            await Task.Run(() => _pairingService.RejectPeer(peer.PeerId));
            DiscoveredPeers.Remove(peer);
            StatusMessage = $"Rejected {peer.DeviceName}";
            Log.Information("Rejected peer: {PeerId}", peer.PeerId);
            await PersistPairingStateAsync();
        }
        catch (Exception ex)
        {
            ErrorMessage = $"Failed to reject peer: {ex.Message}";
            Log.Error(ex, "Failed to reject peer");
        }
    }

    /// <summary>
    /// Removes a trusted peer.
    /// </summary>
    [RelayCommand]
    private async Task RemoveTrustedPeer(TrustedPeer peer)
    {
        try
        {
            await Task.Run(() => _pairingService.RemoveTrustedPeer(peer.PeerId));
            TrustedPeers.Remove(peer);
            StatusMessage = $"Removed {peer.DeviceName}";
            Log.Information("Removed trusted peer: {PeerId}", peer.PeerId);
            await PersistPairingStateAsync();
        }
        catch (Exception ex)
        {
            ErrorMessage = $"Failed to remove peer: {ex.Message}";
            Log.Error(ex, "Failed to remove trusted peer");
        }
    }

    /// <summary>
    /// Clears the sync code and resets to initial state.
    /// </summary>
    [RelayCommand]
    private async Task ClearCode()
    {
        try
        {
            StopDiscoveryTimer();
            await Task.Run(() => _pairingService.ClearSyncCode());
            SyncCode = null;
            SyncCodeInput = string.Empty;
            DiscoveredPeers.Clear();
            CurrentStep = PairingStep.EnterCode;
            StatusMessage = "Enter a sync code or generate a new one";
            Log.Information("Cleared sync code");
            await PersistPairingStateAsync();
        }
        catch (Exception ex)
        {
            ErrorMessage = $"Failed to clear code: {ex.Message}";
            Log.Error(ex, "Failed to clear sync code");
        }
    }

    /// <summary>
    /// Shows the trusted peers list.
    /// </summary>
    [RelayCommand]
    private async Task ShowTrustedPeers()
    {
        var trustedPeers = await Task.Run(() => _pairingService.GetTrustedPeers());
        TrustedPeers.Clear();
        foreach (var peer in trustedPeers)
            TrustedPeers.Add(peer);
        CurrentStep = PairingStep.TrustedPeers;
        StatusMessage = $"{TrustedPeers.Count} trusted device(s)";
    }

    /// <summary>
    /// Goes back to the code entry step.
    /// </summary>
    [RelayCommand]
    private void BackToCode()
    {
        StopDiscoveryTimer();
        CurrentStep = PairingStep.EnterCode;
        StatusMessage = HasSyncCode ? "Sync code active" : "Enter a sync code or generate a new one";
    }

    /// <summary>
    /// Saves the device name.
    /// </summary>
    [RelayCommand]
    private async Task SaveDeviceName()
    {
        try
        {
            if (!string.IsNullOrWhiteSpace(DeviceName))
            {
                var name = DeviceName;
                await Task.Run(() => _pairingService.SetDeviceName(name));
                Log.Information("Set device name: {Name}", DeviceName);
                await PersistPairingStateAsync();
                StatusMessage = "Device name saved";
            }
        }
        catch (Exception ex)
        {
            ErrorMessage = $"Failed to save device name: {ex.Message}";
            Log.Error(ex, "Failed to save device name");
        }
    }

    // ========================================================================
    // Private Methods
    // ========================================================================

    private void LoadInitialState()
    {
        try
        {
            // Load pairing state from persisted settings
            var savedState = _appSettings.Settings.SyncPairingState;
            if (!string.IsNullOrEmpty(savedState))
            {
                try
                {
                    _pairingService.LoadPairingState(savedState);
                    Log.Information("Loaded persisted pairing state");
                }
                catch (Exception ex)
                {
                    Log.Warning(ex, "Failed to load persisted pairing state, starting fresh");
                }
            }

            // Load existing sync code if any
            SyncCode = _pairingService.GetSyncCode();

            // Load device name: persisted user choice takes priority over FFI hostname default
            var persistedName = _appSettings.Settings.SyncDeviceName;
            if (!string.IsNullOrEmpty(persistedName))
            {
                DeviceName = persistedName;
                _pairingService.SetDeviceName(persistedName);
            }
            else
            {
                DeviceName = _pairingService.GetDeviceName();
                if (string.IsNullOrEmpty(DeviceName))
                {
                    DeviceName = Environment.MachineName;
                }
                _pairingService.SetDeviceName(DeviceName);
            }

            // Load trusted peers
            RefreshTrustedPeers();

            if (HasSyncCode)
            {
                StatusMessage = "Sync code active";
            }
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to load initial pairing state");
        }
    }

    private async Task PersistPairingStateAsync()
    {
        try
        {
            var deviceName = DeviceName;
            var stateJson = await Task.Run(() => _pairingService.SavePairingState());
            _appSettings.Settings.SyncPairingState = stateJson;
            _appSettings.Settings.SyncDeviceName = deviceName;
            _appSettings.SaveDebounced();
            Log.Debug("Persisted pairing state");
        }
        catch (Exception ex)
        {
            Log.Warning(ex, "Failed to persist pairing state");
        }
    }

    private void StartDiscoveryTimer()
    {
        StopDiscoveryTimer();

        _discoveryTimer = new System.Timers.Timer(5000); // Check for peers every 5 seconds
        _discoveryTimer.AutoReset = true;
        _discoveryTimer.Elapsed += (_, _) => RefreshDiscoveredPeersFromBackground();
        _discoveryTimer.Start();
    }

    private void StopDiscoveryTimer()
    {
        _discoveryTimer?.Stop();
        _discoveryTimer?.Dispose();
        _discoveryTimer = null;
    }

    /// <summary>
    /// Called from the discovery timer's thread pool thread. Runs FFI off the UI thread,
    /// then dispatches only the UI property updates to the dispatcher.
    /// </summary>
    private void RefreshDiscoveredPeersFromBackground()
    {
        // Skip if a previous tick is still running (mutex contention)
        if (Interlocked.CompareExchange(ref _discoveryRunning, 1, 0) != 0) return;
        try
        {
            var peers = _pairingService.GetPairingDiscoveredPeers();
            var pending = peers
                .Where(p => p.PairingStatus == PairingStatus.PendingLocalApproval)
                .ToList();

            _dispatcher.Post(() =>
            {
                DiscoveredPeers.Clear();
                foreach (var peer in pending)
                    DiscoveredPeers.Add(peer);

                StatusMessage = pending.Count > 0
                    ? $"Found {pending.Count} device(s) - approve to start syncing"
                    : "Searching for devices with the same code...";
            });
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to refresh discovered peers");
        }
        finally
        {
            Interlocked.Exchange(ref _discoveryRunning, 0);
        }
    }

    private void RefreshTrustedPeers()
    {
        try
        {
            var peers = _pairingService.GetTrustedPeers();

            TrustedPeers.Clear();
            foreach (var peer in peers)
            {
                TrustedPeers.Add(peer);
            }
        }
        catch (Exception ex)
        {
            Log.Error(ex, "Failed to refresh trusted peers");
        }
    }

    /// <summary>
    /// Cleans up resources.
    /// </summary>
    public void Dispose()
    {
        StopDiscoveryTimer();
    }
}

/// <summary>
/// Steps in the pairing flow.
/// </summary>
public enum PairingStep
{
    /// <summary>Enter or generate a sync code.</summary>
    EnterCode,
    /// <summary>Discovering and approving peers.</summary>
    Discovery,
    /// <summary>Viewing trusted peers.</summary>
    TrustedPeers
}
