use vessel_control::ControlState;

fn main() {
    let state = ControlState::new();

    println!(
        "VESSEL control plane ready with {} nodes",
        state.node_count(),
    );
}
