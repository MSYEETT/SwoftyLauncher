import struct
import tempfile
import unittest
from pathlib import Path

import heads
from heads import erpw, gfx_material

SHADER = "eaid://shader_package/character/skin.nefx2"
F1_25 = gfx_material.write25(gfx_material.Material(25, SHADER, [
    gfx_material.Param("me_tint", gfx_material.CONST, struct.pack("<f", 1.0), 3, 1, 0, 0),
    gfx_material.Param("me_baseTex", gfx_material.TEXTURE, "eaid://character_package/x_d.tif", 2, 0, 0xffffffff, 0),
], [["PerMaterial", 4]]))
F1_23 = gfx_material.write23(gfx_material.Material(23, SHADER, [
    gfx_material.Param("me_tint", gfx_material.CONST, struct.pack("<f", 1.0)),
    gfx_material.Param("me_baseTex", gfx_material.TEXTURE, "eaid://character_package/x_d.tif"),
    gfx_material.Param("me_base", gfx_material.SAMPLER, "eaid://character_package/x_d.tif.sampler"),
]))
ZSTD_MAGIC = b"\x28\xb5\x2f\xfd"


def write_erp(path, entries):
    e = erpw.Erp.__new__(erpw.Erp)
    e.hdr, e.ver, e.res = b"ERPK" + struct.pack("<I", 3) + bytes(0x28), 3, []
    for name, kind, version, data, comp in entries:
        r, f = erpw.Res(), erpw.Frag()
        f.name, f.flags, f.comp, f.size = b"temp", 0, comp, len(data)
        f.raw = erpw.zstd_c(data) if comp in heads.ZSTD_CODES else data
        f.packed = len(f.raw)
        r.name, r.type, r.unk, r.frags, r.hash = name, kind.ljust(16, b"\0"), struct.pack("<I", version), [f], bytes(16)
        e.res.append(r)
    e.to_file(str(path))


def version_offsets(path):
    b = path.read_bytes()
    out, pos = {}, 0x38
    for r in erpw.Erp(str(path)).res:
        name_len = struct.unpack_from("<H", b, pos + 4)[0]
        out[r.name] = pos + 6 + name_len + 16
        pos += 4 + struct.unpack_from("<I", b, pos)[0]
    return out


class Heads(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / "head.erp"

    def tearDown(self):
        self.dir.cleanup()

    def test_f1_25_materials_get_resource_version_2_and_nothing_else_changes(self):
        write_erp(self.path, [
            ("a/materials/head.material", b"GfxMaterialRes", 1, F1_25, erpw.STORED),
            ("a/materials/eye.material", b"GfxMaterialRes", 1, F1_25, 0x10),
            ("a/materials/arm.material", b"GfxMaterialRes", 1, F1_25, erpw.ZSTD),
            ("a/materials/lashes.material", b"GfxMaterialRes", 1, F1_23, 0x10),
            ("b/materials/head.material", b"GfxMaterialRes", 2, F1_25, erpw.ZSTD),
            ("a/textures/x_d.tif", b"GfxSurfaceRes", 1, b"surface", erpw.STORED),
        ])
        before = self.path.read_bytes()
        self.assertEqual(heads.relabel(self.path), ["head.material", "eye.material", "arm.material"])
        after = self.path.read_bytes()
        self.assertEqual(len(after), len(before))
        at = version_offsets(self.path)
        changed = {at[n] for n in ("a/materials/head.material", "a/materials/eye.material", "a/materials/arm.material")}
        self.assertEqual({i for i in range(len(after)) if after[i] != before[i]}, changed)
        self.assertEqual([after[i:i + 4] for i in sorted(changed)], [heads.VERSION_25] * 3)
        self.assertEqual(heads.relabel(self.path), [])

    def test_a_relabelled_zstd_fragment_stays_a_zstd_frame(self):
        write_erp(self.path, [("a/materials/eye.material", b"GfxMaterialRes", 1, F1_25, 0x10)])
        heads.relabel(self.path)
        f = erpw.Erp(str(self.path)).res[0].frags[0]
        self.assertEqual((f.comp, f.raw[:4], erpw.zstd_d(f.raw)), (0x10, ZSTD_MAGIC, F1_25))

    @unittest.skipUnless(heads.OUT.is_dir(), "heads/ is not built")
    def test_built_heads_hold_zstd_frames_and_version_2_f1_25_materials(self):
        for path in sorted(heads.OUT.rglob("*.erp")):
            for r in erpw.Erp(str(path)).res:
                for f in r.frags:
                    if f.comp in heads.ZSTD_CODES:
                        self.assertEqual(f.raw[:4], ZSTD_MAGIC, f"{path.name}: {r.name}")
                if r.type.rstrip(b"\0") == b"GfxMaterialRes" and heads.f1_25_layout(heads.frag(r, 0)):
                    self.assertEqual(r.unk[:4], heads.VERSION_25, f"{path.name}: {r.name}")


if __name__ == "__main__":
    unittest.main()
