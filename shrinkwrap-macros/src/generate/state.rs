use super::*;
use darling::util::PathList;
use crate::parse::types::{ExtraOpts, GlobalOpts, WrapperOpts};

pub(crate) struct State {
    pub global: GlobalOpts,
    pub wrapper_opts: WrapperOpts,
    pub extra_opts: ExtraOpts,

    pub root_ident: Ident,
    pub default_derives: Vec<Path>,

    pub nest_hierarchy: NestHierarchy,
    pub struct_attr_resolver: StructAttrResolver,
    pub field_resolver: FieldResolver,

    /// Nest ID -> Ident of nest's source data - populated during init
    nest_source_ident: HashMap<String, Ident>,
}

// FIXME: use Rc's
impl State {
    pub fn new(
        global: GlobalOpts,
        wrapper: WrapperOpts,
        extra: ExtraOpts,
        root_ident: Ident,
        nest_hierarchy: NestHierarchy,
        struct_attr_resolver: StructAttrResolver,
        field_resolver: FieldResolver,
    ) -> Self {
        let mut state = Self {
            root_ident: root_ident.clone(),
            default_derives: Self::init_default_derives(&global),
            global,
            wrapper_opts: wrapper,
            extra_opts: extra,
            nest_hierarchy,
            struct_attr_resolver,
            field_resolver,
            nest_source_ident: HashMap::default(),
        };
        let source_idents = state.build_source_idents_map(&state.root_ident);
        state.nest_source_ident = source_idents;

        state
    }
    fn base_derives() -> Vec<Path> {
        vec![
            parse_quote!(::std::fmt::Debug),
            parse_quote!(::std::clone::Clone),
            parse_quote!(::serde::Serialize),
        ]
    }
    fn init_default_derives(global_opts: &GlobalOpts) -> Vec<Path> {
        let mut derives = Self::base_derives();

        // derive `JsonSchema` if either schema or inline attribute flags are set
        if global_opts.schema() {
            derives.push(parse_quote!(::schemars::JsonSchema));
        }

        // add derives defined in global opts
        derives.extend(global_opts.derive_all.to_vec());

        derives
    }

    pub(crate) fn full_derives(&self, custom_derives: PathList) -> Vec<Path> {
        let mut base = self.default_derives.clone();
        base.extend((*custom_derives).clone());
        base
    }

    pub(crate) fn full_struct_attrs(&self, nest_id: Option<&str>, class: StructClass) -> Vec<Attribute> {
        let mut base = Vec::new();
        if self.global.inline() {
            base.push(parse_quote!(#[schemars(inline)]));
        }
        let custom_attrs = self.struct_attr_resolver.resolve(nest_id, class);
        base.extend(custom_attrs);
        base
    }

    pub(crate) fn nest_source_ident(&self, nest_id: &str) -> &Ident {
        self.nest_source_ident
        .get(nest_id)
        .expect_or_abort(format!("Internal macro error - nest_source_ident map missing ID: {nest_id}").as_str())
    }

    fn build_source_idents_map(&self, origin_ident: &Ident) -> HashMap<String, Ident> {
        let mut map = HashMap::new();
        for child in self.nest_hierarchy.get_children(None) {
            self.populate_nest_source_ident(&mut map, child.as_str(), origin_ident);
        }

        map
    }
    fn populate_nest_source_ident(&self, map: &mut HashMap<String, Ident>, nest_id: &str, source_ident: &Ident) {
        map.insert(nest_id.to_string(), source_ident.clone());

        // generate ident/struct name for the dest nest
        let nest_ident = self.nest_hierarchy.get_nest_opts(nest_id).struct_name(source_ident);

        // repeat for each child using newly generated nest ident
        for child in self.nest_hierarchy.get_children(Some(nest_id)) {
            self.populate_nest_source_ident(map, child.as_str(), &nest_ident);
        }
    }
}
