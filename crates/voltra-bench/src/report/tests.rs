use super::*;

fn stats(median: f64) -> Stats {
    Stats {
        median,
        mad: 1.0,
        min: median - 1.0,
        max: median + 1.0,
        samples: 3,
        iterations: 1,
    }
}

fn report(results: Vec<BenchResult>) -> Report {
    Report {
        results,
        ..Report::new()
    }
}

#[test]
fn a_report_reads_back_as_written() {
    let written = report(vec![
        BenchResult::measured("a/b", 10.0, stats(100.0)),
        BenchResult::skipped("gpu/x", 15.0, "no adapter"),
    ]);
    let read = Report::from_json(&written.to_json()).expect("reads");
    assert_eq!(read, written);
}

#[test]
fn another_format_is_refused_by_number() {
    let mut text = report(Vec::new()).to_json();
    text = text.replace(&format!("\"format\": {FORMAT}"), "\"format\": 99");
    assert!(matches!(
        Report::from_json(&text),
        Err(ReportError::Format { found: 99 })
    ));
    assert!(matches!(
        Report::from_json("not json"),
        Err(ReportError::Json(_))
    ));
}

#[test]
fn the_best_of_runs_keeps_the_fastest_median_and_the_spread() {
    let runs = [
        report(vec![BenchResult::measured("a", 10.0, stats(120.0))]),
        report(vec![BenchResult::measured("a", 10.0, stats(100.0))]),
        report(vec![BenchResult::measured("a", 10.0, stats(110.0))]),
    ];
    let best = Report::best_of(&runs).expect("combines");
    let a = best.get("a").expect("kept");
    assert_eq!(a.median(), Some(100.0));
    assert_eq!(a.runs, 3);
    let spread = a.run_spread.expect("several runs");
    assert!((spread - 0.2).abs() < 1e-9, "{spread}");
}

#[test]
fn a_skip_in_one_run_keeps_the_others_measurement() {
    let runs = [
        report(vec![BenchResult::skipped("gpu", 10.0, "no adapter")]),
        report(vec![BenchResult::measured("gpu", 10.0, stats(50.0))]),
    ];
    let best = Report::best_of(&runs).expect("combines");
    assert_eq!(best.get("gpu").and_then(BenchResult::median), Some(50.0));

    let skipped = [report(vec![BenchResult::skipped("gpu", 10.0, "none")])];
    let best = Report::best_of(&skipped).expect("combines");
    assert!(best.get("gpu").expect("kept").skipped.is_some());
}

#[test]
fn reports_of_two_machines_do_not_combine() {
    let mut other = report(Vec::new());
    other.machine.cpus += 1;
    assert!(matches!(
        Report::best_of(&[report(Vec::new()), other]),
        Err(ReportError::Mismatch(_))
    ));
    assert!(matches!(Report::best_of(&[]), Err(ReportError::Empty)));
}

#[test]
fn a_report_writes_and_reads_through_a_file() {
    let dir = std::env::temp_dir().join(format!("voltra-bench-{}", std::process::id()));
    let path = dir.join("nested").join("run.json");
    let written = report(vec![BenchResult::measured("a", 10.0, stats(1.0))]);
    written.write(&path).expect("writes");
    assert_eq!(Report::read(&path).expect("reads"), written);
    let _ = std::fs::remove_dir_all(dir);
}
