pub mod balancer;
pub mod config;
pub mod listener;
pub mod pool;
pub mod router;
pub mod telemetry;

#[test]
fn adder() {
    let result = 2 + 2;
    assert_eq!(result, 4);
}
