"""Exercise workspace opt-out checks and compile deliberately invalid Rust code."""
import json
import os
import re
import tomllib
from pathlib import Path
import subprocess
import tempfile
import unittest

from check_rust_policy import cargo_binary, validate_policy

ROOT = Path(__file__).resolve().parents[1]


class ManifestPolicyTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="desktop-rust-policy-manifest-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "Cargo.toml").write_text((ROOT / "Cargo.toml").read_text())
        (self.root / "rust-toolchain.toml").write_text((ROOT / "rust-toolchain.toml").read_text())
        self.member = self.root / "crates" / "future" / "Cargo.toml"
        self.member.parent.mkdir(parents=True)
        self.member.write_text('[package]\nname = "future"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\nrust-version.workspace = true\n[lints]\nworkspace = true\n')
        self.metadata = {"workspace_members": ["future"], "packages": [{"id": "future", "manifest_path": str(self.member)}]}

    def test_inherited_policy_is_accepted(self):
        self.assertEqual(validate_policy(self.root, self.metadata), [])

    def test_a_new_crate_cannot_silently_opt_out(self):
        self.member.write_text(self.member.read_text().replace('[lints]\nworkspace = true', ''))
        self.assertTrue(any("inherit workspace lints" in error for error in validate_policy(self.root, self.metadata)))

    def test_member_package_override_is_rejected(self):
        self.member.write_text(self.member.read_text().replace('edition.workspace = true', 'edition = "2021"'))
        self.assertTrue(any("package.edition" in error for error in validate_policy(self.root, self.metadata)))

    def test_member_lints_cannot_weaken_workspace_rules(self):
        self.member.write_text(self.member.read_text().replace('[lints]\nworkspace = true', '[lints.rust]\nunsafe_code = "allow"'))
        self.assertTrue(any("inherit workspace lints" in error for error in validate_policy(self.root, self.metadata)))

    def test_unregistered_crate_is_rejected(self):
        orphan = self.root / "crates" / "unchecked" / "Cargo.toml"
        orphan.parent.mkdir()
        orphan.write_text('[package]\nname = "unchecked"\nversion = "0.0.0"\n')
        self.assertTrue(any("not a workspace member" in error for error in validate_policy(self.root, self.metadata)))

    def test_floating_toolchain_is_rejected(self):
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "stable"\ncomponents = ["rustfmt", "clippy"]\n')
        self.assertTrue(any("exact stable release" in error for error in validate_policy(self.root, self.metadata)))

    def test_untested_minimum_toolchain_is_rejected(self):
        path = self.root / "Cargo.toml"
        path.write_text(re.sub(r'^rust-version\s*=\s*"[^"]+"', 'rust-version = "0.0"', path.read_text(), count=1, flags=re.M))
        self.assertTrue(any("minimum Rust version" in error for error in validate_policy(self.root, self.metadata)))


class CompilerPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="desktop-rust-policy-compile-")
        cls.root = Path(cls.temp.name)
        (cls.root / "src").mkdir()
        policy = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["lints"]
        lines = []
        for group, values in policy.items():
            lines.append(f"[workspace.lints.{group}]")
            for name, value in values.items():
                if isinstance(value, dict):
                    encoded = "{ " + ", ".join(f"{key} = {json.dumps(item)}" for key, item in value.items()) + " }"
                else:
                    encoded = json.dumps(value)
                lines.append(f"{json.dumps(name)} = {encoded}")
        lints = "\n".join(lines) + "\n"
        (cls.root / "Cargo.toml").write_text('[package]\nname = "rust-policy-probe"\nversion = "0.0.0"\nedition = "2021"\n[lints]\nworkspace = true\n[workspace]\n' + lints)
        (cls.root / "clippy.toml").write_text((ROOT / "clippy.toml").read_text())
        # The fixture is outside the repository, so select the same pinned compiler explicitly.
        cls.channel = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def compile(self, source):
        (self.root / "src/main.rs").write_text(source)
        env = dict(os.environ)
        for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS"):
            env.pop(key, None)
        env['CARGO_TARGET_DIR'] = str(self.root / 'target')
        result = subprocess.run([cargo_binary(), '+' + self.channel, 'clippy', '--offline', '--quiet', '--all-targets', '--message-format=json', '--', '-D', 'warnings'], cwd=self.root, env=env, text=True, capture_output=True, timeout=90)
        codes = []
        for line in result.stdout.splitlines():
            item = json.loads(line)
            code = item.get('message', {}).get('code')
            if code:
                codes.append(code['code'])
        return result, codes

    def test_production_violations_are_rejected_by_the_actual_compiler(self):
        cases = [
            ('clippy::unwrap_used', 'fn required(x: Option<u8>) -> u8 { x.unwrap() } fn main() { let _ = required(None); }'),
            ('E0453', '#[allow(clippy::unwrap_used, reason = "attempted policy bypass")] fn required(x: Option<u8>) -> u8 { x.unwrap() } fn main() { let _ = required(None); }'),
            ('clippy::expect_used', 'fn required(x: Option<u8>) -> u8 { x.expect("missing") } fn main() { let _ = required(None); }'),
            ('clippy::panic', 'fn main() { panic!("no"); }'),
            ('clippy::todo', 'fn main() { todo!(); }'),
            ('clippy::unimplemented', 'fn main() { unimplemented!(); }'),
            ('clippy::print_stdout', 'fn main() { println!("unexpected"); }'),
            ('missing_docs', 'pub fn undocumented() {} fn main() { undocumented(); }'),
            ('clippy::missing_errors_doc', '/// Fallible operation.\npub fn operation() -> std::io::Result<()> { Ok(()) } fn main() { let _ = operation(); }'),
            ('missing_debug_implementations', '/// Public model.\npub struct Model; fn main() { let _ = Model; }'),
            ('unsafe_code', 'fn main() { let value = 1; let _ = unsafe { std::ptr::read_volatile(&value) }; }'),
            ('clippy::allow_attributes_without_reason', '#[expect(clippy::print_stdout)] fn main() { println!("x"); }'),
            ('unfulfilled_lint_expectations', '#[expect(clippy::print_stdout, reason = "obsolete exception")] fn main() {}'),
        ]
        for expected, source in cases:
            with self.subTest(lint=expected):
                result, codes = self.compile('//! Deliberate compiler policy probe.\n' + source)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(expected, codes, result.stdout + result.stderr)

    def test_scoped_test_assertions_are_permitted(self):
        source = '//! Test-only assertion policy.\nfn main() {}\n#[cfg(test)] mod tests { fn required(x: Option<u8>) -> u8 { x.unwrap() } #[test] fn fixture() { assert_eq!(required(Some(1)), 1); } }'
        result, _ = self.compile(source)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_a_justified_cli_exception_is_permitted(self):
        source = '//! Explicit CLI output.\n#[expect(clippy::print_stdout, reason = "CLI help output")] fn main() { println!("help"); }'
        result, _ = self.compile(source)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == '__main__':
    unittest.main()
