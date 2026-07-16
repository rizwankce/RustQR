import unittest

from scripts import run_competitor_geometry_conformance as geometry


class GeometryProtocolTests(unittest.TestCase):
    def test_canonicalize_normalizes_winding_and_start_corner(self) -> None:
        corners = geometry.canonicalize([(100, 100), (100, 16), (16, 16), (16, 100)])
        self.assertEqual(corners, [(16, 16), (100, 16), (100, 100), (16, 100)])

    def test_iou_is_one_for_equivalent_corner_orders(self) -> None:
        left = [(16, 16), (100, 16), (100, 100), (16, 100)]
        right = geometry.canonicalize([(100, 100), (16, 100), (16, 16), (100, 16)])
        self.assertEqual(geometry.iou(left, right), 1.0)

    def test_zbar_polygon_keeps_raw_payload_and_corners(self) -> None:
        rows = geometry.parse_zbar_polygon(b"+16,+16 +16,+100 +100,+100 +100,+16:abc\n")
        self.assertEqual(rows, [{"payload_hex": "616263", "corners": [(16.0, 16.0), (100.0, 16.0), (100.0, 100.0), (16.0, 100.0)]}])


if __name__ == "__main__":
    unittest.main()
