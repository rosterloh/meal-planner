use std::sync::OnceLock;
use opentelemetry::global;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{
    LogExporter as OtlpLogExporter, SpanExporter as OtlpSpanExporter, WithExportConfig,
    // WithTonicConfig,
};
use opentelemetry_sdk::logs::{BatchLogProcessor, SdkLoggerProvider, SimpleLogProcessor};
use opentelemetry_sdk::trace::{SdkTracer, SdkTracerProvider};
use opentelemetry_sdk::propagation::TraceContextPropagator;
// use tonic::metadata::{MetadataMap, MetadataValue};
// use tonic::transport::ClientTlsConfig;
use tracing_subscriber::{EnvFilter, prelude::*};

use crate::config::TelemetryConfig;

// we maintain an explicit OnceLock for logs because `opentelemetry::global` doesn't expose
// trace/metric-equivalent global logger handlers
static LOGGER_PROVIDER: OnceLock<SdkLoggerProvider> = OnceLock::new();

// see: https://github.com/SigNoz/examples/blob/main/rust/opentelemetry-rust-demo/src/main.rs

// struct AppMetrics {
//     // tracks currently in-flight requests
//     active_requests: UpDownCounter<i64>,
//     // captures end-to-end HTTP request latency (seconds)
//     request_duration: Histogram<f64>,
// }

// initialize global variables to keep duplication to a minimum
// these will be initialized when called, and reused thereafter throughout the application, guaranteeing that
// metrics and tracer objects initialize AFTER the corresponding pipelines have been setup
// static SIGNOZ_HEADERS: OnceLock<HashMap<String, String>> = OnceLock::new();
// SIGNOZ_HEADERS.get_or_init(|| {
//     let mut headers = HashMap::new();
//     if let Ok(key) = std::env::var("SIGNOZ_INGESTION_KEY") {
//         headers.insert("signoz-ingestion-key".to_string(), key);
//     } else {
//         panic!("SIGNOZ_INGESTION_KEY not set");
//     }
//     headers
// });

// we don't need a global meter handle later because we'll reuse instruments directly
// static METRICS: OnceLock<AppMetrics> = OnceLock::new();
// METRICS.get_or_init(|| {
//     let meter = global::meter("opentelemetry-rust-demo");
//     AppMetrics {
//         active_requests: meter
//             .i64_up_down_counter(HTTP_SERVER_ACTIVE_REQUESTS)
//             .with_description("Active HTTP requests")
//             .with_unit("{request}")
//             .build(),
//         request_duration: meter
//             .f64_histogram(HTTP_SERVER_REQUEST_DURATION)
//             .with_description("End-to-end HTTP request duration in seconds")
//             .with_unit("s")
//             // OTel-recommended boundaries for HTTP latency (in seconds)
//             // Without these, all sub-second requests collapse into a single default [0, 5) bucket,
//             // making P95/P99 meaningless
//             .with_boundaries(vec![
//                 0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
//             ])
//             .build(),
//     }
// });

// fn signoz_tonic_metadata() -> MetadataMap {
//     let mut metadata = MetadataMap::new();
//     if let Some(ingestion_key) = SIGNOZ_HEADERS.get("signoz-ingestion-key") {
//         if let Ok(metadata_value) = MetadataValue::try_from(ingestion_key.as_str()) {
//             metadata.insert("signoz-ingestion-key", metadata_value);
//         }
//     }
//     metadata
// }

pub fn init_tracer_provider(config: &TelemetryConfig) -> SdkTracerProvider {
    // set the propagator to be used for extracting and injecting trace context; extraction and injection won't work
    // unless propagators are defined globally first
    global::set_text_map_propagator(opentelemetry::propagation::TextMapCompositePropagator::new(
        vec![Box::new(TraceContextPropagator::new())],
    ));

    // use gRPC exporter with TLS and metadata headers for SigNoz cloud
    let otlp_endpoint = config.otlp_endpoint.clone();
    let otlp_exporter = OtlpSpanExporter::builder()
        .with_tonic()
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_endpoint(otlp_endpoint)
        // .with_tls_config(ClientTlsConfig::new().with_native_roots())
        // .with_metadata(signoz_tonic_metadata())
        .build()
        .unwrap();

    let provider = SdkTracerProvider::builder()
        // * enable the console exporter for debugging
        // .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
        .with_batch_exporter(otlp_exporter)
        .build();

    global::set_tracer_provider(provider.clone());
    provider
}

// pub fn init_meter_provider(config: &TelemetryConfig) {
//     let otlp_endpoint = config.otlp_endpoint.clone();

//     // the reader object controls how often metrics are exported
//     // let stdout_reader = PeriodicReader::builder(opentelemetry_stdout::MetricExporter::default())
//     //     .with_interval(Duration::from_secs(5))
//     //     .build();

//     let otlp_exporter = opentelemetry_otlp::MetricExporter::builder()
//         .with_tonic()
//         .with_protocol(opentelemetry_otlp::Protocol::Grpc)
//         .with_endpoint(otlp_endpoint)
//         .with_tls_config(ClientTlsConfig::new().with_native_roots())
//         .with_metadata(signoz_tonic_metadata())
//         .build()
//         .expect("Failed to create OTLP exporter");

//     // the interval should be high enough to avoid overloading the backend but low enough for accurate analysis
//     let otlp_reader = PeriodicReader::builder(otlp_exporter)
//         .with_interval(Duration::from_secs(30))
//         .build();

//     let provider = SdkMeterProvider::builder()
//         // * enable stdout reader for debugging
//         // .with_reader(stdout_reader)
//         .with_reader(otlp_reader)
//         .build();
//     global::set_meter_provider(provider);
// }

pub fn init_logger_provider(config: &TelemetryConfig) {
    let otlp_endpoint = config.otlp_endpoint.clone();
    let otlp_exporter = OtlpLogExporter::builder()
        .with_tonic()
        .with_protocol(opentelemetry_otlp::Protocol::Grpc)
        .with_endpoint(otlp_endpoint)
        // .with_tls_config(ClientTlsConfig::new().with_native_roots())
        // .with_metadata(signoz_tonic_metadata())
        .build()
        .unwrap();

    let provider = SdkLoggerProvider::builder()
        // * enable the console log processor for debugging
        .with_log_processor(SimpleLogProcessor::new(
            opentelemetry_stdout::LogExporter::default(),
        ))
        .with_log_processor(BatchLogProcessor::builder(otlp_exporter).build())
        .build();

    // store the provider in a OnceLock as there is no global logger setter API equivalent to tracer/meter globals
    let _ = LOGGER_PROVIDER.set(provider);
}

pub fn init_tracing_subscriber(tracer: SdkTracer) {
    // filter noisy logs from dependencies
    let filter = EnvFilter::new(
        "info,meal_planner=debug,opentelemetry_sdk=warn,opentelemetry_otlp=warn,opentelemetry_http=warn,reqwest=warn,hyper_util=warn,hyper=warn,h2=warn,tonic=warn",
    );
    let logger_provider = LOGGER_PROVIDER
        .get()
        .expect("logger provider should be initialised before tracing subscriber");
    // bridge tracing events -> OTel logs
    let otel_log_layer = OpenTelemetryTracingBridge::new(logger_provider);
    // bridge tracing spans -> OTel traces
    let otel_span_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(otel_span_layer)
        .with(otel_log_layer)
        .init();
}