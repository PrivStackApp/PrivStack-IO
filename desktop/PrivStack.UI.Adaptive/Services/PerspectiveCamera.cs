// ============================================================================
// File: PerspectiveCamera.cs
// Description: Orbit camera for 3D embedding space visualization.
//              Supports yaw/pitch/distance orbiting and perspective projection.
// ============================================================================

using Avalonia;

namespace PrivStack.UI.Adaptive.Services;

public sealed class PerspectiveCamera
{
    public double Yaw { get; set; }
    public double Pitch { get; set; } = 0.3;
    public double Distance { get; set; } = 5.0;
    public double Fov { get; set; } = 600.0;

    public double TargetX { get; set; }
    public double TargetY { get; set; }
    public double TargetZ { get; set; }

    // Auto-rotation speed (radians per tick)
    public double AutoRotateSpeed { get; set; } = 0.003;
    public bool IsAutoRotating { get; set; } = true;

    private const double MinPitch = -Math.PI / 2.0 + 0.1;
    private const double MaxPitch = Math.PI / 2.0 - 0.1;
    private const double MinDistance = 0.5;
    private const double MaxDistance = 50.0;

    public void Orbit(double deltaYaw, double deltaPitch)
    {
        Yaw += deltaYaw;
        Pitch = Math.Clamp(Pitch + deltaPitch, MinPitch, MaxPitch);
    }

    public void Zoom(double delta)
    {
        Distance = Math.Clamp(Distance * (1.0 - delta * 0.1), MinDistance, MaxDistance);
    }

    public void Tick()
    {
        if (IsAutoRotating)
            Yaw += AutoRotateSpeed;
    }

    /// <summary>
    /// Camera position in world space derived from spherical coordinates.
    /// </summary>
    public (double X, double Y, double Z) GetPosition()
    {
        var cosP = Math.Cos(Pitch);
        return (
            TargetX + Distance * cosP * Math.Sin(Yaw),
            TargetY + Distance * Math.Sin(Pitch),
            TargetZ + Distance * cosP * Math.Cos(Yaw)
        );
    }

    /// <summary>
    /// Projects a 3D world point to 2D screen coordinates relative to viewport center.
    /// Returns (screenX, screenY, depth) where depth > 0 means in front of camera.
    /// </summary>
    public (double ScreenX, double ScreenY, double Depth) WorldToScreen(
        double wx, double wy, double wz, double viewportCenterX, double viewportCenterY)
    {
        var (cx, cy, cz) = GetPosition();

        // Translate to camera space
        var dx = wx - cx;
        var dy = wy - cy;
        var dz = wz - cz;

        // Camera forward vector (from camera toward target)
        var cosP = Math.Cos(Pitch);
        var fx = -cosP * Math.Sin(Yaw);
        var fy = -Math.Sin(Pitch);
        var fz = -cosP * Math.Cos(Yaw);

        // Camera right vector (cross product of up and forward)
        var rx = Math.Cos(Yaw);
        var rz = -Math.Sin(Yaw);

        // Camera up vector (cross of forward and right)
        var ux = fy * rz;
        var uy = fz * rx - fx * rz;
        var uz = -fy * rx;

        // Project onto camera axes
        var camX = dx * rx + dz * rz;
        var camY = dx * ux + dy * uy + dz * uz;
        var camZ = dx * fx + dy * fy + dz * fz;

        if (camZ <= 0.01) // Behind camera (positive camZ = in front)
            return (0, 0, -1);

        var screenX = viewportCenterX + Fov * camX / camZ;
        var screenY = viewportCenterY - Fov * camY / camZ;

        return (screenX, screenY, camZ);
    }
}
