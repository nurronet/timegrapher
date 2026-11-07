"""Offline tests of docs_html.py's link handling (rendering needs pandoc and
runs in the Release workflow's docs job)."""

import tempfile
import unittest
from pathlib import Path

import docs_html

REF = "0123abc"


def guide_chapters():
    # Listed, not named, so CI's light-run rule does not count them as read.
    return sorted(path.relative_to(docs_html.ROOT).as_posix()
                  for path in (docs_html.ROOT / docs_html.GUIDE).glob("[0-9]*.md"))


def other_docs():
    # Every Markdown page under docs/ outside the user guide.
    return sorted(path.relative_to(docs_html.ROOT).as_posix()
                  for path in (docs_html.ROOT / "docs").rglob("*.md")
                  if not path.is_relative_to(docs_html.ROOT / docs_html.GUIDE))


class Outputs(unittest.TestCase):
    def test_the_download_carries_the_start_page_guide_and_licence(self):
        mapping = docs_html.outputs("macos")
        self.assertEqual(mapping["packaging/macos/start-here.md"], "Start Here.html")
        self.assertEqual(mapping["LICENSE"], "LICENSE.txt")
        self.assertEqual(mapping[f"{docs_html.GUIDE}/README.md"], "User Guide/index.html")
        chapters = guide_chapters()
        self.assertGreater(len(chapters), 20)
        for page in chapters:
            self.assertEqual(mapping[page], "User Guide/" + page.rsplit("/", 1)[1][:-3] + ".html")
        for source, dest in mapping.items():
            self.assertTrue((docs_html.ROOT / source).is_file(), source)
            self.assertTrue(dest in ("Start Here.html", "LICENSE.txt")
                            or dest.startswith("User Guide/"), dest)

    def test_nothing_else_in_docs_is_carried(self):
        mapping = docs_html.outputs("linux")
        others = other_docs()
        self.assertTrue(others)
        for page in others:
            self.assertNotIn(page, mapping)
        self.assertNotIn("README.md", mapping)
        self.assertFalse([s for s in mapping
                          if s.startswith("docs/") and not s.startswith(docs_html.GUIDE + "/")])

    def test_the_documentation_stays_small(self):
        # Each download carries it.
        mapping = docs_html.outputs("linux")
        size = sum((docs_html.ROOT / s).stat().st_size for s in mapping)
        self.assertLess(size, 20_000_000)

    def test_each_platform_has_a_start_page(self):
        for platform in docs_html.PLATFORMS:
            self.assertTrue((docs_html.ROOT / f"packaging/{platform}/start-here.md").is_file())


class RewriteLinks(unittest.TestCase):
    def rewrite(self, href, source=None):
        source = source or f"{docs_html.GUIDE}/README.md"
        mapping = docs_html.outputs("linux")
        out = docs_html.rewrite_links(f'<a href="{href}">x</a>', source,
                                      mapping[source], mapping, REF)
        return out[len('<a href="'):-len('">x</a>')]

    def test_guide_pages_link_to_the_pages_beside_them(self):
        first, second = guide_chapters()[:2]
        name = second.rsplit("/", 1)[1]
        self.assertEqual(self.rewrite(name), name[:-3] + ".html")
        self.assertEqual(self.rewrite(f"{name}#a-heading"), name[:-3] + ".html#a-heading")
        self.assertEqual(self.rewrite("README.md", first), "index.html")
        image = sorted((docs_html.ROOT / docs_html.GUIDE / "images").glob("*.png"))[0].name
        self.assertEqual(self.rewrite(f"images/{image}", first), f"images/{image}")

    def test_other_docs_link_to_github(self):
        page = other_docs()[0]
        rel = "../" + page[len("docs/"):]
        self.assertEqual(self.rewrite(rel),
                         f"https://github.com/{docs_html.REPO}/blob/{REF}/{page}")

    def test_a_link_to_a_missing_file_fails_check(self):
        self.assertEqual(self.rewrite("no-such-page.md"), "no-such-page.md")
        with tempfile.TemporaryDirectory() as tmp:
            page = Path(tmp, "User Guide/index.html")
            page.parent.mkdir()
            mapping = docs_html.outputs("linux")
            page.write_text(docs_html.rewrite_links(
                '<a href="no-such-page.md">x</a>', f"{docs_html.GUIDE}/README.md",
                "User Guide/index.html", mapping, REF), encoding="utf-8")
            missing, _ = docs_html.check(tmp)
        self.assertEqual(missing, ["User Guide/index.html: no-such-page.md"])

    def test_start_page_links_into_the_guide(self):
        chapter = guide_chapters()[1]
        self.assertEqual(self.rewrite(f"../../{chapter}", "packaging/linux/start-here.md"),
                         "User%20Guide/" + chapter.rsplit("/", 1)[1][:-3] + ".html")
        self.assertEqual(self.rewrite(f"../../{docs_html.GUIDE}/README.md",
                                      "packaging/linux/start-here.md"),
                         "User%20Guide/index.html")

    def test_back_to_the_start_page_quotes_the_space(self):
        chapter = guide_chapters()[0]
        mapping = docs_html.outputs("linux")
        out = docs_html.rewrite_links('<a href="../../packaging/linux/start-here.md">x</a>',
                                      chapter, mapping[chapter], mapping, REF)
        self.assertIn('href="../Start%20Here.html"', out)

    def test_other_repository_files_go_to_github_at_the_commit(self):
        self.assertEqual(
            self.rewrite("../../crates/timegrapher-core/src/measure.rs"),
            f"https://github.com/{docs_html.REPO}/blob/{REF}/crates/timegrapher-core/src/measure.rs")
        self.assertEqual(self.rewrite("../../crates"),
                         f"https://github.com/{docs_html.REPO}/tree/{REF}/crates")

    def test_web_links_and_anchors_are_kept(self):
        self.assertEqual(self.rewrite("https://example.com/a.md"), "https://example.com/a.md")
        self.assertEqual(self.rewrite("#install"), "#install")


class Check(unittest.TestCase):
    def test_reports_missing_files_and_headings(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / "Guide").mkdir()
            (folder / "Start Here.html").write_text(
                '<a href="Guide/a.html#b">ok</a><a href="Guide/gone.html">x</a>'
                '<a href="Guide/a.html#nope">y</a><a href="https://e.com/">z</a>')
            (folder / "Guide/a.html").write_text('<h2 id="b">B</h2><a href="#b">self</a>')
            missing, unknown = docs_html.check(folder)
            self.assertEqual(missing, ["Start Here.html: Guide/gone.html"])
            self.assertEqual(unknown, ["Start Here.html: Guide/a.html#nope"])

    def test_links_to_other_folders_are_not_followed(self):
        with tempfile.TemporaryDirectory() as tmp, tempfile.TemporaryDirectory() as other:
            Path(other, "broken.html").write_text('<a href="gone.html">x</a>')
            Path(tmp, "Start Here.html").write_text("<p>hi</p>")
            try:
                Path(tmp, "Applications").symlink_to(other, target_is_directory=True)
            except OSError:
                self.skipTest("this system cannot make symbolic links")
            self.assertEqual(docs_html.check(tmp), ([], []))

    def test_an_empty_folder_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(docs_html.check(tmp)[0], ["no pages"])


if __name__ == "__main__":
    unittest.main()
