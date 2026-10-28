import io
import json
import shutil
import unittest
from contextlib import redirect_stdout
from pathlib import Path

from helpers import FIXTURES, TempRegistries

import stdschema as S
import validate as V


def errors(schema, value, patch=False):
    return S.Validator(schema).errors(value, patch=patch)


class Subset(unittest.TestCase):
    def test_types(self):
        self.assertEqual(errors({"type": "integer"}, 3), [])
        self.assertEqual(errors({"type": "integer"}, 3.0), [])
        self.assertTrue(errors({"type": "integer"}, 3.5))
        self.assertTrue(errors({"type": "integer"}, True))
        self.assertTrue(errors({"type": "number"}, False))
        self.assertEqual(errors({"type": "number"}, 3), [])
        self.assertEqual(errors({"type": ["string", "null"]}, None), [])
        self.assertTrue(errors({"type": "string"}, None))

    def test_enum_and_const(self):
        self.assertEqual(errors({"enum": [1, "a"]}, 1.0), [])
        self.assertTrue(errors({"enum": [1]}, True))
        self.assertTrue(errors({"const": "a"}, "b"))

    def test_ranges(self):
        s = {"type": "number", "minimum": 1, "maximum": 3, "exclusiveMaximum": 3}
        self.assertEqual(errors(s, 2), [])
        self.assertTrue(errors(s, 0))
        self.assertTrue(errors(s, 3))

    def test_strings(self):
        s = {"type": "string", "minLength": 2, "maxLength": 3, "pattern": "^[a-z]+$"}
        self.assertEqual(errors(s, "ab"), [])
        self.assertTrue(errors(s, "a"))
        self.assertTrue(errors(s, "abcd"))
        self.assertTrue(errors(s, "AB"))

    def test_objects(self):
        s = {"type": "object", "required": ["a"], "properties": {"a": {"type": "integer"}}, "additionalProperties": False}
        self.assertEqual(errors(s, {"a": 1}), [])
        self.assertEqual(errors(s, {}), [("/a", "is required")])
        self.assertEqual(errors(s, {"a": 1, "b": 2}), [("/b", "is not a field of this record")])
        self.assertEqual(errors({"type": "object"}, {"anything": [1]}), [])

    def test_arrays(self):
        s = {"type": "array", "items": {"type": "integer"}, "minItems": 1, "maxItems": 2, "uniqueItems": True}
        self.assertEqual(errors(s, [1, 2]), [])
        self.assertTrue(errors(s, []))
        self.assertTrue(errors(s, [1, 2, 3]))
        self.assertTrue(errors(s, [1, 1]))
        self.assertEqual(errors(s, ["x"])[0][0], "/0")

    def test_combinators(self):
        self.assertEqual(errors({"anyOf": [{"type": "string"}, {"type": "integer"}]}, 1), [])
        self.assertTrue(errors({"anyOf": [{"type": "string"}, {"type": "integer"}]}, None))
        self.assertTrue(errors({"oneOf": [{"type": "number"}, {"type": "integer"}]}, 1))
        self.assertTrue(errors({"allOf": [{"minimum": 1}, {"maximum": 0}]}, 1))

    def test_refs(self):
        s = {"$defs": {"n": {"type": "integer"}}, "type": "object", "properties": {"a": {"$ref": "#/$defs/n"}}}
        self.assertEqual(errors(s, {"a": 1}), [])
        self.assertTrue(errors(s, {"a": "x"}))
        self.assertIn("schema error", errors({"$ref": "#/nope"}, 1)[0][1])

    def test_record_refs(self):
        plain = {"type": "string", "x-ref": "driver.identity"}
        for value in ("game/lewis_hamilton", "swofty.classic-seasons-2017/f1_2017", "venue/sochi?", "venue/sepang|venue/bahrain",
                      "venue/a|venue/b?"):
            self.assertEqual(errors(plain, value), [], value)
        for value in ("lewis_hamilton", "game/", "/x", "game/a b", "game/a?|game/b", "game/*", "Game/x"):
            self.assertTrue(errors(plain, value), value)
        strict = dict(plain, **{"x-missing": {"default": "required"}})
        self.assertEqual(errors(strict, "game/a"), [])
        self.assertTrue(errors(strict, "game/a?"))
        self.assertTrue(errors(strict, "game/a|game/b"))
        fallback = dict(plain, **{"x-missing": {"default": "fallback", "to": "/donor"}})
        self.assertEqual(errors(fallback, "game/a|game/b"), [])
        self.assertTrue(errors({"x-ref": "driver.identity"}, 3))

    def test_keyed(self):
        s = {"type": "array", "x-key": ["round", "slot"],
             "items": {"type": "object", "required": ["round", "slot", "kind"],
                       "properties": {"round": {"type": "integer"}, "slot": {"type": "integer"}, "kind": {"type": "string"}}}}
        self.assertEqual(errors(s, [{"round": 1, "slot": 1, "kind": "a"}, {"round": 1, "slot": 2, "kind": "a"}]), [])
        self.assertIn("repeats key", errors(s, [{"round": 1, "slot": 1, "kind": "a"}, {"round": 1, "slot": 1.0, "kind": "b"}])[0][1])
        self.assertIn("no key field slot", errors(s, [{"round": 1, "kind": "a"}])[0][1])
        self.assertIn("holds objects", errors(s, [3])[0][1])
        self.assertEqual(errors(s, [{"round": 1, "slot": 1}], patch=True), [])
        self.assertTrue(errors(s, [{"round": 1, "slot": 1}]))
        self.assertEqual(errors(s, [{"round": 1, "slot": 1, "$delete": True}], patch=True), [])
        self.assertTrue(errors(s, [{"round": 1, "slot": 1, "$delete": False}], patch=True))
        self.assertTrue(errors(s, [{"round": 1, "slot": 1, "kind": "x", "$delete": True}], patch=True))
        self.assertTrue(errors(s, [{"round": 1, "slot": 1, "kind": "x", "$delete": True}]))

    def test_patch(self):
        s = {"type": "object", "required": ["a"], "properties": {"a": {"type": "integer"}, "b": {"type": "integer"},
             "c": {"type": "array", "maxItems": 1, "items": {"type": "object", "required": ["x"]}}}}
        self.assertEqual(errors(s, {"b": 1}, patch=True), [])
        self.assertEqual(errors(s, {"b": None}, patch=True), [])
        self.assertEqual(errors(s, {"a": None}, patch=True), [("/a", "null removes a required field")])
        self.assertTrue(errors(s, {"c": [{}]}, patch=True), "a list that isn't keyed is replaced whole")
        self.assertTrue(errors(s, {"c": [{"x": 1}, {"x": 2}]}, patch=True))


class Examples(TempRegistries):
    def run_examples(self):
        return V.run_examples(self.root, "registries")

    def test_fixture_examples(self):
        entries = self.run_examples()
        self.assertTrue(entries)
        self.assertEqual([e["file"] for e in entries if not e["ok"]], [])
        self.assertEqual({e["expect"] for e in entries}, {"valid", "invalid"})

    def test_kinds_and_versions(self):
        entries = {Path(e["file"]).name: e for e in self.run_examples()}
        self.assertEqual(entries["2016.json"]["kind"], "record")
        self.assertEqual(sorted(entries["2016.json"]["versions"]), ["1.0", "1.1"])
        self.assertEqual(sorted(entries["2017@2.json"]["versions"]), ["2.0"])
        self.assertEqual(sorted(entries["reserve@1.1.json"]["versions"]), ["1.1"])
        self.assertEqual(entries["records.json"]["kind"], "records")
        self.assertEqual(entries["whatif-layer.json"]["kind"], "layers")
        self.assertEqual(sorted(entries["select-layers.json"]["versions"]), ["1.0", "1.1"])
        self.assertEqual(sorted(entries["promote@1.0.json"]["versions"]), ["1.0"])

    def test_invalid_example_fails(self):
        shutil.copy(self.reg / "examples/must-fail/slot-out-of-range.json", self.reg / "examples/slot.json")
        bad = [e for e in self.run_examples() if not e["ok"]]
        self.assertEqual([Path(e["file"]).name for e in bad], ["slot.json"])
        self.assertIn("12 is above the maximum 11", "\n".join(V.describe(bad[0])))

    def test_must_fail_that_passes(self):
        shutil.copy(self.reg / "examples/2016.json", self.reg / "examples/must-fail/2016.json")
        bad = [e for e in self.run_examples() if not e["ok"]]
        self.assertEqual([Path(e["file"]).name for e in bad], ["2016.json"])
        self.assertEqual(bad[0]["expect"], "invalid")
        self.assertIn("accepted by 1.0, 1.1", "\n".join(V.describe(bad[0])))

    def test_must_fail_accepted_by_one_minor(self):
        shutil.copy(self.reg / "examples/must-fail/promote@1.0.json", self.reg / "examples/must-fail/promote.json")
        bad = [e for e in self.run_examples() if not e["ok"]]
        self.assertEqual([e["accepted_by"] for e in bad], [["1.1"]])

    def test_missing_major(self):
        shutil.copy(self.reg / "examples/2016.json", self.reg / "examples/2016@3.json")
        bad = [e for e in self.run_examples() if not e["ok"]]
        self.assertIn("career.season has no major 3", "\n".join(V.describe(bad[0])))

    def test_missing_minor_in_must_fail(self):
        shutil.copy(self.reg / "examples/must-fail/promote@1.0.json", self.reg / "examples/must-fail/promote@1.7.json")
        bad = [e for e in self.run_examples() if not e["ok"]]
        self.assertEqual([Path(e["file"]).name for e in bad], ["promote@1.7.json"])

    def test_example_of_another_registry(self):
        (self.reg / "examples/other.json").write_text(json.dumps({"vehicle": {"x": {}}}))
        bad = [e for e in self.run_examples() if not e["ok"]]
        self.assertEqual([Path(e["file"]).name for e in bad], ["other.json"])

    def test_not_json(self):
        (self.reg / "examples/must-fail/broken.json").write_text("{")
        bad = [e for e in self.run_examples() if not e["ok"]]
        self.assertIn("not JSON", "\n".join(V.describe(bad[0])))

    def test_json_report(self):
        out = io.StringIO()
        with redirect_stdout(out):
            code = V.main(["--repo", str(self.tmp), "--registries", "registries", "--json"])
        self.assertEqual(code, 0)
        report = json.loads(out.getvalue())
        self.assertEqual(report["failed"], 0)
        entry = next(e for e in report["examples"] if e["file"].endswith("must-fail/slot-out-of-range.json"))
        self.assertEqual(entry["versions"]["1.0"], [["/teams/0/slot", "12 is above the maximum 11"]])

    def test_cli_lines(self):
        out = io.StringIO()
        with redirect_stdout(out):
            code = V.main(["--repo", str(self.tmp), "--registries", "registries"])
        self.assertEqual(code, 0)
        self.assertIn("examples: 6 valid, 20 must-fail, 0 wrong", out.getvalue())

    def test_schema_mode(self):
        out = io.StringIO()
        with redirect_stdout(out):
            ok = V.main(["--schema", str(self.reg / "1.0.json"), str(self.reg / "examples/2016.json")])
            bad = V.main(["--schema", str(self.reg / "1.0.json"), str(self.reg / "examples/must-fail/wrong-type.json")])
        self.assertEqual((ok, bad), (0, 1))

    def test_data_mode_skips_addon_registries(self):
        f = self.tmp / "sounds.json"
        f.write_text(json.dumps({"tails.whatif-history.notes": {"a": {"x": 1}},
                                 "career.season": {"what_if": json.loads((self.reg / "examples/2016.json").read_text())}}))
        out = io.StringIO()
        with redirect_stdout(out):
            code = V.main(["--repo", str(self.tmp), "--registries", "registries", "--data", str(f)])
        self.assertEqual(code, 0, out.getvalue())
        self.assertIn("(1.0, 1.1)", out.getvalue())


class Agreement(unittest.TestCase):
    def test_every_fixture_has_a_result_per_version(self):
        for e in V.run_examples(FIXTURES / "registries", "registries"):
            self.assertTrue(e["versions"] or e.get("rejections"), e["file"])


if __name__ == "__main__":
    unittest.main()
