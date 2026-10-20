"""Control icons must work without the build machine's or desktop's icon theme."""
import json
from pathlib import Path
import re
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]


class IconAssetsTests(unittest.TestCase):
    def test_every_explicit_control_icon_is_embedded_and_attributed(self):
        used = {
            name
            for source in (ROOT / "src").rglob("*.rs")
            for name in re.findall(r'"([a-z][a-z0-9-]*-symbolic)"', source.read_text())
        }
        self.assertTrue(used)
        self.assertTrue(all(name.startswith("papo-") for name in used))
        resources = ET.parse(ROOT / "assets/papo.gresource.xml").getroot()
        entries = {
            Path(entry.attrib["alias"]).stem: entry.text
            for entry in resources.findall("gresource/file")
            if entry.attrib.get("alias", "").startswith("scalable/actions/")
        }
        self.assertEqual(used, entries.keys())
        provenance = json.loads((ROOT / "assets/icons/symbolic/licenses/sources.json").read_text())
        self.assertEqual(used, {entry["name"] for entry in provenance})
        for name, source in entries.items():
            with self.subTest(icon=name):
                svg = ET.parse(ROOT / "assets" / source).getroot()
                self.assertEqual(svg.tag, "{http://www.w3.org/2000/svg}svg")
                for dimension in ("width", "height"):
                    self.assertGreater(float(svg.attrib[dimension].removesuffix("px")), 0)
                    self.assertLessEqual(float(svg.attrib[dimension].removesuffix("px")), 32)


if __name__ == "__main__":
    unittest.main()
