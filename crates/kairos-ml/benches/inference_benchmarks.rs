//! # Performance Benchmarks for Kairos ML
//!
//! This module contains criterion-based benchmarks for measuring:
//! - Model inference latency
//! - Feature extraction throughput
//! - Training throughput
//!
//! ## Running Benchmarks
//!
//! ```bash
//! # Run all benchmarks
//! cargo bench -p kairos-ml
//!
//! # Run specific benchmark
//! cargo bench -p kairos-ml -- inference_single
//! ```
//!
//! ## Performance Targets
//!
//! | Metric | Target | Description |
//! |--------|--------|-------------|
//! | Inference latency | < 10ms | Single prediction on CPU |
//! | Feature extraction | < 5ms | 100 studies per candle |
//! | Training throughput | > 1000 samples/s | Training speed on GPU |

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

/// Benchmark configuration
const INFERENCE_BATCH_SIZES: &[usize] = &[1, 4, 16, 64];
const FEATURE_COUNTS: &[usize] = &[3, 10, 50, 100];
const LOOKBACK_PERIODS: &[usize] = &[10, 20, 50, 100];

/// Benchmark inference latency for a single prediction
///
/// Target: < 10ms per prediction on CPU
#[allow(dead_code)]
pub fn benchmark_inference_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("inference_single");

    // Note: Actual benchmark requires tch to be initialized
    // This is a placeholder that demonstrates the benchmark structure

    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(2));

    for batch_size in INFERENCE_BATCH_SIZES {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &batch_size| {
                // Placeholder: actual benchmark would call model.predict()
                // b.iter(|| {
                //     let input = create_test_tensor(batch_size, input_size);
                //     let _ = model.predict(&input);
                // });

                // Simulate some work to show benchmark structure
                black_box(0u64);
            },
        );
    }

    group.finish();
}

/// Benchmark inference latency with different input sizes
#[allow(dead_code)]
pub fn benchmark_inference_input_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("inference_input_sizes");

    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    for lookback in LOOKBACK_PERIODS {
        for features in FEATURE_COUNTS {
            let input_size = lookback * features;
            let id = format!("{}_{}", lookback, features);

            group.bench_with_input(
                BenchmarkId::from_parameter(&id),
                &(lookback, features),
                |b, _| {
                    // Placeholder for actual benchmark
                    black_box(input_size);
                },
            );
        }
    }

    group.finish();
}

/// Benchmark feature extraction throughput
///
/// Target: < 5ms for 100 studies per candle
#[allow(dead_code)]
pub fn benchmark_feature_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_extraction");

    group.sample_size(50);
    group.measurement_time(Duration::from_secs(5));

    for num_studies in &[10, 50, 100, 200] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_studies),
            num_studies,
            |b, &num_studies| {
                // Placeholder for actual benchmark
                // b.iter(|| {
                //     let mut extractor = StudyFeatureExtractor::new(config.clone());
                //     for _ in 0..num_studies {
                //         extractor.add_scalar("study", 0.0, 1000);
                //     }
                //     let _ = extractor.extract(20);
                // });
                black_box(num_studies);
            },
        );
    }

    group.finish();
}

/// Benchmark dataset generation throughput
///
/// Target: > 1000 samples/second
#[allow(dead_code)]
pub fn benchmark_dataset_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dataset_generation");

    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    for num_candles in &[1000, 10000, 100000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_candles),
            num_candles,
            |b, &num_candles| {
                // Placeholder for actual benchmark
                // b.iter(|| {
                //     let candles = generate_test_candles(num_candles);
                //     let studies = compute_test_studies(&candles);
                //     let _ = DataGenerator::generate(&candles, &studies, &config, &label_config);
                // });
                black_box(num_candles);
            },
        );
    }

    group.finish();
}

/// Benchmark training loop throughput
#[allow(dead_code)]
pub fn benchmark_training_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("training_throughput");

    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    for batch_size in &[32, 64, 128, 256] {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                // Placeholder for actual benchmark
                // b.iter(|| {
                //     let result = train(&config, &dataset, &callback);
                // });
                black_box(batch_size);
            },
        );
    }

    group.finish();
}

/// Benchmark normalization performance
#[allow(dead_code)]
pub fn benchmark_normalization(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalization");

    group.sample_size(100);
    group.measurement_time(Duration::from_secs(5));

    for num_values in &[100, 1000, 10000, 100000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_values),
            num_values,
            |b, &num_values| {
                // Placeholder for actual benchmark
                // b.iter(|| {
                //     let values: Vec<f64> = (0..num_values).map(|i| (i as f64).sin()).collect();
                //     let _ = normalize(&values, NormalizationMethod::ZScore);
                // });
                black_box(num_values);
            },
        );
    }

    group.finish();
}

/// Benchmark model loading time
#[allow(dead_code)]
pub fn benchmark_model_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("model_loading");

    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));

    // Note: Only one model size for loading benchmark
    group.bench_function("load_model_1m_params", |b| {
        // Placeholder for actual benchmark
        // b.iter(|| {
        //     let model = TchModel::load("trained_model.pt");
        // });
        black_box(());
    });

    group.finish();
}

// Configuration for benchmarks
criterion_group!(
    benches,
    benchmark_inference_single,
    benchmark_inference_input_sizes,
    benchmark_feature_extraction,
    benchmark_dataset_generation,
    benchmark_training_throughput,
    benchmark_normalization,
    benchmark_model_loading,
);

criterion_main!(benches);
