use super::*;

#[test]
fn writer_admission_remains_held_until_diagnostics_are_cleared() {
    let gate = SqliteWriteGate::default();
    let guard = gate.try_lock().unwrap();
    let holder = gate.0.holder.lock().unwrap();
    let dropping = std::thread::spawn(move || drop(guard));
    let deadline = Instant::now() + Duration::from_millis(100);
    let mut acquired_early = false;
    while Instant::now() < deadline {
        if gate.0.mutex.try_lock().is_ok() {
            acquired_early = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(holder);
    dropping.join().unwrap();
    assert!(
        !acquired_early,
        "writer admission opened before holder cleanup"
    );
}

#[tokio::test]
async fn reacquisition_attributes_diagnostics_to_the_new_holder() {
    let gate = SqliteWriteGate::default();
    let first_line = line!() + 1;
    let guard = gate.lock().await;
    assert_eq!(gate.0.holder.lock().unwrap().unwrap().line(), first_line);
    assert!(gate.try_lock().is_none());
    drop(guard);
    assert!(gate.0.holder.lock().unwrap().is_none());
    let second_line = line!() + 1;
    let guard = gate.try_lock().unwrap();
    assert_eq!(gate.0.holder.lock().unwrap().unwrap().line(), second_line);
    drop(guard);
    assert!(gate.0.holder.lock().unwrap().is_none());
}
