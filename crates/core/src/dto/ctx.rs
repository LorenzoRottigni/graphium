#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CtxAccess {
    None,
    Ref,
    Mut,
}

#[cfg_attr(feature = "export", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CtxAccessDto {
    #[default]
    None,
    Ref,
    Mut,
}

impl From<CtxAccess> for CtxAccessDto {
    fn from(value: CtxAccess) -> Self {
        match value {
            CtxAccess::None => Self::None,
            CtxAccess::Ref => Self::Ref,
            CtxAccess::Mut => Self::Mut,
        }
    }
}
