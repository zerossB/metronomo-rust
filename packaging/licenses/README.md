# License supplements

These files supplement notices absent from published crate archives. Each directory
identifies a locked crate/version and includes `provenance.json` with source URLs,
revision information when available, and SHA256 hashes of the stored texts.

Most texts were recovered from the exact `.cargo_vcs_info.json` commit. Exceptions:

- `simd_helpers 0.1.0`: MIT text from the same upstream repository at the recorded
  later revision; its original publishing commit had no license file.
- `mac 0.1.1` and `pathfinder_geometry 0.5.1`: standard Apache-2.0 text, selected
  from the crate's declared `MIT/Apache-2.0` alternatives, plus published author
  attribution. The upstream file/commit could not supply a license text.
- `seahash 4.1.0`: standard MIT text plus authors as declared in the published
  Cargo manifest; neither the archive nor the pinned upstream path supplied a text.

Standard texts do not fabricate an upstream copyright notice. The project MIT
license does not replace these terms. Revisit these records when dependencies
change. `scripts/release/third_party_notices.py` assembles the supplements with license
and notice files from the resolved normal/build dependency source trees, including
nested font/icon notices, without network access.
