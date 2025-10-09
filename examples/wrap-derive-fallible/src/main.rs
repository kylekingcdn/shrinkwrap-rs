use schemars::JsonSchema;
use serde::Serialize;
use shrinkwrap::{TryToWrappedWith, Transform, TryTransformToNest, Wrap};

// -- transform

struct MyTransformOpts {
    with_text: bool,
    with_value: bool,
}
struct MyTransform {}
impl Transform for MyTransform {
    type Options = MyTransformOpts;
}

#[derive(Debug)]
pub struct MyError {

}

// -- data definition

/// Docs on origin `TestData`
#[derive(Debug, Clone, Serialize, Wrap, JsonSchema, PartialEq)]
#[allow(clippy::duplicated_attributes)]
#[shrinkwrap(schema, inline, transform = MyTransform, all_optional, derive_all(PartialEq), fallible(error = MyError))]
#[shrinkwrap(nest(id = "text", field_type = String))]
#[shrinkwrap(nest(id = "value", field_type = f32))]
#[shrinkwrap(nest(id = "value_text", field_name = "text", field_type = String, nested(origin = ApiDataNestedValue)))]
#[shrinkwrap_attr(limit(class(nest)), attr(serde(rename_all = "SCREAMING_SNAKE_CASE")))]
pub struct ApiData {
    #[shrinkwrap(nests("text", "value", "value_text"))]
    #[shrinkwrap_attr(attr(schemars(with = "String")), limit(nests("value")))]
    pub balance: f32,

    #[shrinkwrap(nests("text"))]
    #[shrinkwrap_attr(
        limit(nests("text")),
        attr(doc = "Timestamp in format: 'YYYY-MM-DD hh:MM:SS'")
    )]
    pub last_modified: u128,
}

// -- transform conversions

impl TryTransformToNest<Option<ApiDataNestedText>> for MyTransform {
    type Data = ApiData;
    type Error = MyError;

    fn try_transform_to_nest(
        &self,
        data: &ApiData,
        options: &MyTransformOpts,
    ) -> Result<Option<ApiDataNestedText>, MyError> {
        Ok(options.with_text.then_some(ApiDataNestedText {
            balance: format!("{:.2} credits", data.balance),
            last_modified: "2025-02-24 12:20:49".into(), // used as an example, fake impl
        }))
    }
}
impl TryTransformToNest<Option<ApiDataNestedValue>> for MyTransform {
    type Data = ApiData;
    type Error = MyError;

    fn try_transform_to_nest(
        &self,
        _data: &ApiData,
        options: &MyTransformOpts,
    ) -> Result<Option<ApiDataNestedValue>, MyError> {
        Ok(options
            .with_value
            .then_some(ApiDataNestedValue { balance: 6.66f32 }))
    }
}
impl TryTransformToNest<Option<ApiDataNestedValueText>> for MyTransform {
    type Data = ApiDataNestedValue;
    type Error = MyError;

    fn try_transform_to_nest(
        &self,
        data: &ApiDataNestedValue,
        options: &MyTransformOpts,
    ) -> Result<Option<ApiDataNestedValueText>, MyError> {
        Ok(options.with_text.then_some(ApiDataNestedValueText {
            balance: format!("${:.2} USD", data.balance),
        }))
    }
}

// -- integration example

pub fn main() -> Result<(), serde_json::Error> {
    println!("Starting example: example-derive");
    let data = ApiData {
        balance: 82.231_21,
        last_modified: 1754443805,
    };

    let global_transform = MyTransform {};
    let transform_opts = MyTransformOpts {
        with_value: true,
        with_text: true,
    };

    let wrapped = data.try_to_wrapped_with(&global_transform, &transform_opts);
    println!("Generated wrapper via transform: {wrapped:#?}");

    // note: shrinkwrap inline flag has renamed the wrapper and inlined all child data
    let wrapped_schema = schemars::schema_for_value!(wrapped).to_value();
    let wrapped_schema_json = serde_json::to_string_pretty(&wrapped_schema)?;
    println!("Wrapper schema: {wrapped_schema_json}");

    let wrapped_json = serde_json::to_string_pretty(&wrapped)?;
    println!("Serialized wrapper: {wrapped_json}");

    Ok(())
}
