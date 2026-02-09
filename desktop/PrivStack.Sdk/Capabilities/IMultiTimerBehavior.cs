using System.Collections.Generic;

namespace PrivStack.Sdk.Capabilities;

/// <summary>
/// Lightweight snapshot of one active timer, projected for the shell sidebar.
/// </summary>
public record ActiveTimerInfo(string ItemId, string Title, string ElapsedDisplay, bool IsRunning);

/// <summary>
/// Extended timer capability that supports multiple concurrent timers.
/// The shell discovers this via <see cref="ICapabilityBroker"/> and renders
/// an ItemsControl of active timers in the sidebar.
/// </summary>
public interface IMultiTimerBehavior : ITimerBehavior
{
    IReadOnlyList<ActiveTimerInfo> ActiveTimers { get; }
    int ActiveTimerCount { get; }

    void PauseTimer(string itemId);
    void ResumeTimer(string itemId);
    void StopTimer(string itemId);
}
