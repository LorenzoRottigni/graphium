pub trait Node<Ctx> {
    fn run(ctx: &mut Ctx);
}
