use graphio_macro::graph;

// Example: nodes with sequential (>>) and parallel (&) execution
// This executes as:
// 1. get_data
// 2. validate_data (after get_data)
// 3. normalize_data (after validate_data)
// 4. print_data, send_email, publish_event (all parallel)
// 5. disconnect_from_db (after parallel group)
graph! {
    name: data_pipeline,
    nodes: [get_data >> validate_data >> normalize_data >> print_data & send_email & publish_event >> disconnect_from_db]
}