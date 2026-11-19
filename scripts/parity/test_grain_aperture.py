"""Check aperture placement and light integration against analytic flat fields."""
import unittest

import numpy as np

from grain_aperture import aperture_readings


class ApertureReadingsTest(unittest.TestCase):
    def test_split_field_and_flat_neighbour(self):
        for pitch in (3, 6):
            with self.subTest(pitch=pitch):
                side = 48 // pitch
                density = np.zeros((side, 2 * side))
                density[:, side//2:side] = 2
                density[:, side:] = 1
                expected = [[-np.log10((1 + 0.01) / 2), 1]]
                np.testing.assert_allclose(aperture_readings(density, pitch), expected, atol=1e-12)

    def test_outside_disk_has_no_effect(self):
        density = np.zeros((16, 16))
        density[0, 0] = 2
        np.testing.assert_allclose(aperture_readings(density, 3), [[0]], atol=1e-12)


if __name__ == "__main__":
    unittest.main()
