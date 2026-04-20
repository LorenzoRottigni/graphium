use graphium_macro::{graph, node};

pub fn main() {
    let mut ctx = graphium::Context::default();

    node! {

        fn duplicate(a: u32) -> (u32, u32) {
            (a, a)
        }
    }

    node! {
        fn get_number() -> u32 {
            42
        }
    }

    node! {
        fn pipe_number(a: u32) -> u32 {
            a
        }
    }

    graph! {
        #[metadata(outputs = (a_split: u32))]
        #[metrics("performance")]
        OwnedGraph {
            GetNumber() -> (number) >>
            Duplicate(number) -> (a_split, b_split) >>
            PipeNumber(a_split) -> (a_split)
        }
    }
    let duplicated = OwnedGraph::__graphium_run(&mut ctx);

    assert_eq!(duplicated, 42);
}

/*
fn expanded() {
     pub struct OwnedGraph; impl OwnedGraph
{
    fn __graphium_graph_metrics() -> & 'static :: graphium :: metrics ::
    GraphMetricsHandle
    {
        static METRICS : :: std :: sync :: OnceLock < :: graphium :: metrics
        :: GraphMetricsHandle > = :: std :: sync :: OnceLock :: new();
        METRICS.get_or_init(||
        {
            :: graphium :: metrics ::
            graph_metrics(stringify! (OwnedGraph), module_path! (), ::
            graphium :: metrics :: MetricConfig
            {
                performance : true, errors : false, count : false, caller :
                false, success_rate : false, fail_rate : false,
            },)
        })
    } pub fn run(ctx : & mut :: graphium :: Context)
    {
        < Self as :: graphium :: Graph < :: graphium :: Context >> ::
        run(ctx);
    } pub fn __graphium_run(ctx : & mut :: graphium :: Context,) -> u32
    {
        let __graphium_metrics = Self :: __graphium_graph_metrics(); let
        __graphium_start = __graphium_metrics.start_timer(); let value =
        {
            let mut __graphium_captured_12_a_split = :: std :: option ::
            Option :: None;
            {
                let mut __graphium_captured_7_a_split = :: std :: option ::
                Option :: None; let mut __graphium_captured_8_b_split = :: std
                :: option :: Option :: None;
                {
                    let mut __graphium_hop_0_number = :: std :: option :: Option
                    :: Some(GetNumber :: __graphium_run(ctx,)); let mut
                    __graphium_payload_1_number =
                    __graphium_hop_0_number.take(); let __graphium_arg_2_number
                    =
                    __graphium_payload_1_number.take().unwrap_or_else(|| panic!
                    (concat! ("missing artifact `", stringify! (number), "`")));
                    let (__graphium_ret_3_a_split, __graphium_ret_4_b_split) =
                    Duplicate :: __graphium_run(ctx, __graphium_arg_2_number);
                    let mut __graphium_hop_5_a_split = :: std :: option ::
                    Option :: Some(__graphium_ret_3_a_split); let mut
                    __graphium_hop_6_b_split = :: std :: option :: Option ::
                    Some(__graphium_ret_4_b_split);
                    __graphium_captured_7_a_split = __graphium_hop_5_a_split;
                    __graphium_captured_8_b_split = __graphium_hop_6_b_split;
                } let mut __graphium_payload_9_a_split =
                __graphium_captured_7_a_split.take(); let
                __graphium_arg_10_a_split =
                __graphium_payload_9_a_split.take().unwrap_or_else(|| panic!
                (concat! ("missing artifact `", stringify! (a_split), "`")));
                let mut __graphium_hop_11_a_split = :: std :: option :: Option
                ::
                Some(PipeNumber ::
                __graphium_run(ctx, __graphium_arg_10_a_split));
                __graphium_captured_12_a_split = __graphium_hop_11_a_split;
            }
            __graphium_captured_12_a_split.take().unwrap_or_else(|| panic!
            (concat! ("missing graph output `", "a_split", "`")))
        }; __graphium_metrics.record_success(__graphium_start); value
    }
    #[doc =
    r" Convenience async entry point that executes the graph directly."] pub
    async fn run_async(ctx : & mut :: graphium :: Context)
    {
        panic!
        (concat!
        ("graph `", stringify! (OwnedGraph),
        "` has explicit inputs/outputs; call it as a nested step: `",
        stringify! (OwnedGraph), "(...) -> (...)`"));
    } pub async fn __graphium_run_async(ctx : & mut :: graphium :: Context,)
    -> u32
    {
        let __graphium_metrics = Self :: __graphium_graph_metrics(); let
        __graphium_start = __graphium_metrics.start_timer(); let value =
        {
            let mut __graphium_captured_25_a_split = :: std :: option ::
            Option :: None;
            {
                let mut __graphium_captured_20_a_split = :: std :: option ::
                Option :: None; let mut __graphium_captured_21_b_split = ::
                std :: option :: Option :: None;
                {
                    let mut __graphium_hop_13_number = :: std :: option ::
                    Option ::
                    Some(GetNumber :: __graphium_run_async(ctx,).await); let mut
                    __graphium_payload_14_number =
                    __graphium_hop_13_number.take(); let
                    __graphium_arg_15_number =
                    __graphium_payload_14_number.take().unwrap_or_else(|| panic!
                    (concat! ("missing artifact `", stringify! (number), "`")));
                    let (__graphium_ret_16_a_split, __graphium_ret_17_b_split) =
                    Duplicate ::
                    __graphium_run_async(ctx, __graphium_arg_15_number).await;
                    let mut __graphium_hop_18_a_split = :: std :: option ::
                    Option :: Some(__graphium_ret_16_a_split); let mut
                    __graphium_hop_19_b_split = :: std :: option :: Option ::
                    Some(__graphium_ret_17_b_split);
                    __graphium_captured_20_a_split = __graphium_hop_18_a_split;
                    __graphium_captured_21_b_split = __graphium_hop_19_b_split;
                } let mut __graphium_payload_22_a_split =
                __graphium_captured_20_a_split.take(); let
                __graphium_arg_23_a_split =
                __graphium_payload_22_a_split.take().unwrap_or_else(|| panic!
                (concat! ("missing artifact `", stringify! (a_split), "`")));
                let mut __graphium_hop_24_a_split = :: std :: option :: Option
                ::
                Some(PipeNumber ::
                __graphium_run_async(ctx, __graphium_arg_23_a_split).await);
                __graphium_captured_25_a_split = __graphium_hop_24_a_split;
            }
            __graphium_captured_25_a_split.take().unwrap_or_else(|| panic!
            (concat! ("missing graph output `", "a_split", "`")))
        }; __graphium_metrics.record_success(__graphium_start); value
    } pub fn graph_def() -> :: graphium :: GraphDef
    {
        :: graphium :: GraphDef
        {
            name : stringify! (OwnedGraph), steps : vec!
            [:: graphium :: GraphStep :: Node
            {
                name : stringify! (GetNumber), inputs : vec! [], outputs :
                vec! [stringify! (number)],
            }, :: graphium :: GraphStep :: Node
            {
                name : stringify! (Duplicate), inputs : vec!
                [stringify! (number)], outputs : vec!
                [stringify! (a_split), stringify! (b_split)],
            }, :: graphium :: GraphStep :: Node
            {
                name : stringify! (PipeNumber), inputs : vec!
                [stringify! (a_split)], outputs : vec! [stringify! (a_split)],
            }],
        }
    }
} impl :: graphium :: Graph < :: graphium :: Context > for OwnedGraph
{
    fn run(ctx : & mut :: graphium :: Context)
    {
        panic!
        (concat!
        ("graph `", stringify! (OwnedGraph),
        "` has explicit inputs/outputs; call it as a nested step: `",
        stringify! (OwnedGraph), "(...) -> (...)`"));
    }
} impl :: graphium :: GraphDefProvider for OwnedGraph
{ fn graph_def() -> :: graphium :: GraphDef { Self :: graph_def() } }
}

fn expanded() {
    pub struct OwnedGraph;
    impl OwnedGraph {
        pub fn run(ctx: &mut ::graphium::Context) {
            <Self as ::graphium::Graph<::graphium::Context>>::run(ctx);
        }
        pub fn __graphium_run(ctx: &mut ::graphium::Context) -> u32 {
            {
                let mut __graphium_captured_12_a_split = ::std::option::Option::None;
                {
                    let mut __graphium_captured_7_a_split = ::std::option::Option::None;
                    let mut __graphium_captured_8_b_split = ::std::option::Option::None;
                    {
                        let mut __graphium_hop_0_number =
                            ::std::option::Option::Some(GetNumber::__graphium_run(ctx));
                        let mut __graphium_payload_1_number = __graphium_hop_0_number.take();
                        let __graphium_arg_2_number =
                            __graphium_payload_1_number.take().unwrap_or_else(|| {
                                panic!(concat!("missing artifact `", stringify!(number), "`"))
                            });
                        let (__graphium_ret_3_a_split, __graphium_ret_4_b_split) =
                            Duplicate::__graphium_run(ctx, __graphium_arg_2_number);
                        let mut __graphium_hop_5_a_split =
                            ::std::option::Option::Some(__graphium_ret_3_a_split);
                        let mut __graphium_hop_6_b_split =
                            ::std::option::Option::Some(__graphium_ret_4_b_split);
                        __graphium_captured_7_a_split = __graphium_hop_5_a_split;
                        __graphium_captured_8_b_split = __graphium_hop_6_b_split;
                    }
                    let mut __graphium_payload_9_a_split = __graphium_captured_7_a_split.take();
                    let __graphium_arg_10_a_split =
                        __graphium_payload_9_a_split.take().unwrap_or_else(|| {
                            panic!(concat!("missing artifact `", stringify!(a_split), "`"))
                        });
                    let mut __graphium_hop_11_a_split = ::std::option::Option::Some(
                        PipeNumber::__graphium_run(ctx, __graphium_arg_10_a_split),
                    );
                    __graphium_captured_12_a_split = __graphium_hop_11_a_split;
                }
                __graphium_captured_12_a_split
                    .take()
                    .unwrap_or_else(|| panic!(concat!("missing graph output `", "a_split", "`")))
            }
        }
        #[doc = r" Convenience async entry point that executes the graph directly."]
        pub async fn run_async(ctx: &mut ::graphium::Context) {
            panic!(concat!(
                "graph `",
                stringify!(OwnedGraph),
                "` has explicit inputs/outputs; call it as a nested step: `",
                stringify!(OwnedGraph),
                "(...) -> (...)`"
            ));
        }
        pub async fn __graphium_run_async(ctx: &mut ::graphium::Context) -> u32 {
            {
                let mut __graphium_captured_25_a_split = ::std::option::Option::None;
                {
                    let mut __graphium_captured_20_a_split = ::std::option::Option::None;
                    let mut __graphium_captured_21_b_split = ::std::option::Option::None;
                    {
                        let mut __graphium_hop_13_number =
                            ::std::option::Option::Some(GetNumber::__graphium_run_async(ctx).await);
                        let mut __graphium_payload_14_number = __graphium_hop_13_number.take();
                        let __graphium_arg_15_number =
                            __graphium_payload_14_number.take().unwrap_or_else(|| {
                                panic!(concat!("missing artifact `", stringify!(number), "`"))
                            });
                        let (__graphium_ret_16_a_split, __graphium_ret_17_b_split) =
                            Duplicate::__graphium_run_async(ctx, __graphium_arg_15_number).await;
                        let mut __graphium_hop_18_a_split =
                            ::std::option::Option::Some(__graphium_ret_16_a_split);
                        let mut __graphium_hop_19_b_split =
                            ::std::option::Option::Some(__graphium_ret_17_b_split);
                        __graphium_captured_20_a_split = __graphium_hop_18_a_split;
                        __graphium_captured_21_b_split = __graphium_hop_19_b_split;
                    }
                    let mut __graphium_payload_22_a_split = __graphium_captured_20_a_split.take();
                    let __graphium_arg_23_a_split =
                        __graphium_payload_22_a_split.take().unwrap_or_else(|| {
                            panic!(concat!("missing artifact `", stringify!(a_split), "`"))
                        });
                    let mut __graphium_hop_24_a_split = ::std::option::Option::Some(
                        PipeNumber::__graphium_run_async(ctx, __graphium_arg_23_a_split).await,
                    );
                    __graphium_captured_25_a_split = __graphium_hop_24_a_split;
                }
                __graphium_captured_25_a_split
                    .take()
                    .unwrap_or_else(|| panic!(concat!("missing graph output `", "a_split", "`")))
            }
        }
        pub fn graph_def() -> ::graphium::GraphDef {
            ::graphium::GraphDef {
                name: stringify!(OwnedGraph),
                steps: vec![
                    ::graphium::GraphStep::Node {
                        name: stringify!(GetNumber),
                        inputs: vec![],
                        outputs: vec![stringify!(number)],
                    },
                    ::graphium::GraphStep::Node {
                        name: stringify!(Duplicate),
                        inputs: vec![stringify!(number)],
                        outputs: vec![stringify!(a_split), stringify!(b_split)],
                    },
                    ::graphium::GraphStep::Node {
                        name: stringify!(PipeNumber),
                        inputs: vec![stringify!(a_split)],
                        outputs: vec![stringify!(a_split)],
                    },
                ],
            }
        }
    }
    impl ::graphium::Graph<::graphium::Context> for OwnedGraph {
        fn run(ctx: &mut ::graphium::Context) {
            panic!(concat!(
                "graph `",
                stringify!(OwnedGraph),
                "` has explicit inputs/outputs; call it as a nested step: `",
                stringify!(OwnedGraph),
                "(...) -> (...)`"
            ));
        }
    }
    impl ::graphium::GraphDefProvider for OwnedGraph {
        fn graph_def() -> ::graphium::GraphDef {
            Self::graph_def()
        }
    }
}
*/
