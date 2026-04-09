use graphio_macro::graph;


graph! {
    name: data_pipeline,
    nodes: [get_data, validate_data, normalize_data, print_data, send_email, publish_event, disconnect_from_db]
}