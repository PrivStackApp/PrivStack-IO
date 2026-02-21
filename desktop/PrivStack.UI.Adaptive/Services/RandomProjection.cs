// ============================================================================
// File: RandomProjection.cs
// Description: Johnson-Lindenstrauss random projection from 768-dim to 3D.
//              Uses a deterministic seeded random matrix for reproducibility.
// ============================================================================

namespace PrivStack.UI.Adaptive.Services;

public static class RandomProjection
{
    private const int SourceDim = 768;
    private const int TargetDim = 3;
    private const int Seed = 42;

    private static readonly double[,] _matrix = GenerateMatrix();

    private static double[,] GenerateMatrix()
    {
        var rng = new Random(Seed);
        var matrix = new double[SourceDim, TargetDim];
        var scale = 1.0 / Math.Sqrt(TargetDim);

        for (int i = 0; i < SourceDim; i++)
        {
            for (int j = 0; j < TargetDim; j++)
            {
                // Box-Muller transform for Gaussian random values
                double u1 = 1.0 - rng.NextDouble();
                double u2 = rng.NextDouble();
                matrix[i, j] = Math.Sqrt(-2.0 * Math.Log(u1)) * Math.Cos(2.0 * Math.PI * u2) * scale;
            }
        }

        return matrix;
    }

    /// <summary>
    /// Projects a 768-dim embedding to 3D coordinates.
    /// </summary>
    public static (double X, double Y, double Z) Project(double[] embedding)
    {
        double x = 0, y = 0, z = 0;
        var dim = Math.Min(embedding.Length, SourceDim);

        for (int i = 0; i < dim; i++)
        {
            x += embedding[i] * _matrix[i, 0];
            y += embedding[i] * _matrix[i, 1];
            z += embedding[i] * _matrix[i, 2];
        }

        return (x, y, z);
    }
}
