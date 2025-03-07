use std::collections::BTreeMap;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

// https://github.com/llvm/llvm-project/blob/c0db8d50ca3ceb1301b2ade2fb86c591a5b64e5c/llvm/tools/llvm-cov/CoverageExporterJson.cpp#L13-L47
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
#[allow(unreachable_pub)]
pub struct LlvmCovJsonExport {
    /// List of one or more export objects
    pub(crate) data: Vec<Export>,
    // llvm.coverage.json.export
    #[serde(rename = "type")]
    pub(crate) type_: String,
    pub(crate) version: String,
}

/// Files -> list of uncovered lines.
pub(crate) type UncoveredLines = BTreeMap<String, Vec<u64>>;

impl LlvmCovJsonExport {
    #[allow(unreachable_pub, dead_code)]
    pub fn demangle(&mut self) {
        for data in &mut self.data {
            if let Some(functions) = &mut data.functions {
                for func in functions {
                    func.name = format!("{:#}", rustc_demangle::demangle(&func.name));
                }
            }
        }
    }

    /// Gets the minimal lines coverage of all files.
    #[allow(unreachable_pub, dead_code)]
    pub fn get_lines_percent(&self) -> Result<f64> {
        let mut count = 0_f64;
        let mut covered = 0_f64;
        for data in &self.data {
            let totals = &data.totals.as_object().context("totals is not an object")?;
            let lines = &totals["lines"].as_object().context("no lines")?;
            count += lines["count"].as_f64().context("no count")?;
            covered += lines["covered"].as_f64().context("no covered")?;
        }

        if count == 0_f64 {
            return Ok(0_f64);
        }

        Ok(covered * 100_f64 / count)
    }

    /// Gets the list of uncovered lines of all files.
    #[allow(unreachable_pub, dead_code)]
    #[must_use]
    pub fn get_uncovered_lines(&self) -> UncoveredLines {
        let mut uncovered_files: UncoveredLines = BTreeMap::new();
        for data in &self.data {
            for file in &data.files {
                // Detect lines which have matching segments but all of them are has_count and
                // count is 0.
                let file_name = &file.filename;
                if let Some(ref segments) = file.segments {
                    let mut lines: BTreeMap<u64, bool> = BTreeMap::new();
                    // Check all segments and see if lines are covered at least once.
                    for segment in segments {
                        let line = segment.0;
                        let count = segment.2;
                        let has_count = segment.3;
                        if !has_count {
                            continue;
                        }

                        // If any segments covers the line, the line is covered.
                        *lines.entry(line).or_insert(false) |= count > 0;
                    }

                    let uncovered_lines: Vec<u64> = lines
                        .iter()
                        .filter(|(_line, covered)| !*covered)
                        .map(|(line, _covered)| *line)
                        .collect();
                    if !uncovered_lines.is_empty() {
                        uncovered_files.insert(file_name.clone(), uncovered_lines);
                    }
                }
            }
        }
        uncovered_files
    }
}

/// Json representation of one `CoverageMapping`
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct Export {
    /// List of objects describing coverage for files
    pub(crate) files: Vec<File>,
    /// List of objects describing coverage for functions
    ///
    /// This is None if report is summary-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) functions: Option<Vec<Function>>,
    pub(crate) totals: serde_json::Value,
}

/// Coverage for a single file
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct File {
    /// List of Branches in the file
    ///
    /// This is None if report is summary-only.
    // https://github.com/llvm/llvm-project/blob/c0db8d50ca3ceb1301b2ade2fb86c591a5b64e5c/llvm/tools/llvm-cov/CoverageExporterJson.cpp#L93
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) branches: Option<Vec<serde_json::Value>>,
    /// List of expansion records
    ///
    /// This is None if report is summary-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) expansions: Option<Vec<serde_json::Value>>,
    pub(crate) filename: String,
    /// List of Segments contained in the file
    ///
    /// This is None if report is summary-only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) segments: Option<Vec<Segment>>,
    /// Object summarizing the coverage for this file
    pub(crate) summary: Summary,
}

/// Describes a segment of the file with a counter
// https://github.com/llvm/llvm-project/blob/c0db8d50ca3ceb1301b2ade2fb86c591a5b64e5c/llvm/tools/llvm-cov/CoverageExporterJson.cpp#L80
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct Segment(
    /* Line */ pub(crate) u64,
    /* Col */ pub(crate) u64,
    /* Count */ pub(crate) u64,
    /* HasCount */ pub(crate) bool,
    /* IsRegionEntry */ pub(crate) bool,
    /* IsGapRegion */ pub(crate) bool,
);

// https://github.com/llvm/llvm-project/blob/c0db8d50ca3ceb1301b2ade2fb86c591a5b64e5c/llvm/tools/llvm-cov/CoverageExporterJson.cpp#L259
/// Coverage info for a single function
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct Function {
    pub(crate) branches: Vec<serde_json::Value>,
    pub(crate) count: u64,
    /// List of filenames that the function relates to
    pub(crate) filenames: Vec<String>,
    pub(crate) name: String,
    pub(crate) regions: Vec<Region>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct Region(
    /* LineStart */ pub(crate) u64,
    /* ColumnStart */ pub(crate) u64,
    /* LineEnd */ pub(crate) u64,
    /* ColumnEnd */ pub(crate) u64,
    /* ExecutionCount */ pub(crate) u64,
    /* FileID */ pub(crate) u64,
    /* ExpandedFileID */ pub(crate) u64,
    /* Kind */ pub(crate) u64,
);

/// Object summarizing the coverage for this file
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct Summary {
    /// Object summarizing branch coverage
    pub(crate) branches: CoverageCounts,
    /// Object summarizing function coverage
    pub(crate) functions: CoverageCounts,
    pub(crate) instantiations: CoverageCounts,
    /// Object summarizing line coverage
    pub(crate) lines: CoverageCounts,
    /// Object summarizing region coverage
    pub(crate) regions: CoverageCounts,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub(crate) struct CoverageCounts {
    pub(crate) count: u64,
    pub(crate) covered: u64,
    // Currently only branches and regions has this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) notcovered: Option<u64>,
    pub(crate) percent: f64,
}

#[cfg(test)]
mod tests {
    use fs_err as fs;

    use super::*;

    #[test]
    fn parse_llvm_cov_json() {
        let files: Vec<_> = glob::glob(&format!(
            "{}/tests/fixtures/coverage-reports/**/*.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
        assert!(!files.is_empty());

        for file in files {
            let s = fs::read_to_string(file).unwrap();
            let json = serde_json::from_str::<LlvmCovJsonExport>(&s).unwrap();
            assert_eq!(json.type_, "llvm.coverage.json.export");
            assert!(json.version.starts_with("2.0."));
            serde_json::to_string(&json).unwrap();
        }
    }

    #[test]
    fn test_get_lines_percent() {
        // There are 5 different percentages, make sure we pick the correct one.
        let file = format!(
            "{}/tests/fixtures/coverage-reports/no_coverage/no_coverage.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let s = fs::read_to_string(file).unwrap();
        let json = serde_json::from_str::<LlvmCovJsonExport>(&s).unwrap();

        let percent = json.get_lines_percent().unwrap();

        let error_margin = f64::EPSILON;
        assert!((percent - 69.565_217_391_304_34).abs() < error_margin);
    }

    #[test]
    fn test_get_uncovered_lines() {
        // Given a coverage report which includes segments:
        // There are 5 different percentages, make sure we pick the correct one.
        let file = format!("{}/tests/fixtures/show-missing-lines.json", env!("CARGO_MANIFEST_DIR"));
        let s = fs::read_to_string(file).unwrap();
        let json = serde_json::from_str::<LlvmCovJsonExport>(&s).unwrap();

        // When finding uncovered lines in that report:
        let uncovered_lines = json.get_uncovered_lines();

        // Then make sure the file / line data matches the expected value:
        let expected: UncoveredLines =
            vec![("src/lib.rs".to_string(), vec![22, 26]), ("src/m.rs".to_string(), vec![5])]
                .into_iter()
                .collect();
        assert_eq!(uncovered_lines, expected);
    }
}
