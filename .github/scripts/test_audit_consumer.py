import unittest
from audit_consumer import registry_violations


class RegistryConsumerTests(unittest.TestCase):
    def metadata(self, sources):
        return {"resolve": {"root": "consumer"}, "packages": [{"id": "consumer", "source": None}] + [{"id": name, "source": source} for name, source in sources.items()]}

    def test_path_and_git_sources_cannot_masquerade_as_clean_consumers(self):
        metadata = self.metadata({"local-path": None, "git-fork": "git+https://example.invalid/fork", "registry": "registry+https://github.com/rust-lang/crates.io-index"})
        self.assertEqual(registry_violations(metadata), ["local-path", "git-fork"])

    def test_root_package_is_the_only_allowed_path_source(self):
        self.assertEqual(registry_violations(self.metadata({"registry": "registry+https://github.com/rust-lang/crates.io-index"})), [])
