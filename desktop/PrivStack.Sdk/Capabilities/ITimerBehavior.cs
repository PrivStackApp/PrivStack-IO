using System.ComponentModel;

namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Capability interface for plugins that expose a running timer to the shell sidebar.
/// The shell discovers providers via <see cref="ICapabilityBroker"/> and forwards
/// property changes to its own UI bindings.
/// </summary>
public interface ITimerBehavior : INotifyPropertyChanged
{
    bool IsTimerActive { get; }
    bool IsTimerRunning { get; }
    string TimerDisplay { get; }
    string? TimedItemTitle { get; }

    void PauseTimer();
    void ResumeTimer();
    void StopTimer();

    /// <summary>
    /// Persists timer state on graceful shutdown so it can be restored on next launch.
    /// </summary>
    void SaveOnShutdown();

    /// <summary>
    /// Restores a previously persisted timer state.
    /// </summary>
    void RestoreState(object? savedState);
}
