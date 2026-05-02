use schemars::JsonSchema;
use serde::Serialize;
use shrinkwrap::{Transform, TryToWrappedWith, TryTransformToNest, Wrap};

// !- Transform

struct MyTransformOpts {
    with_text: bool,
    with_value: bool,
}

struct MyTransform;
impl Transform for MyTransform {
    type Options = MyTransformOpts;
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct MyError;

// !- Data definition

/// Docs on origin `TestData`
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Wrap)]
#[shrinkwrap(schema, inline, transform = MyTransform, all_optional, fallible(error = MyError))]
#[shrinkwrap(nest(id = "text", field_type = String))]
#[shrinkwrap(nest(id = "value", field_type = f32))]
// value_text is branched under the value nest. we manually rename the nest key to text for consistency (see json output)
#[shrinkwrap(nest(id = "value_text", field_name = "text", field_type = String, chain_from = "value"))]
#[shrinkwrap_attr(
    attr(serde(rename_all = "SCREAMING_SNAKE_CASE")), // add generated struct attribute,
    limit(nests("value"), class(wrapper)),            // but restrict assignment to by `wrapper` struct type
)]                                                    // and by associated nest `value` (matches only `ApiDataNestedValueWrapper`)
pub struct ApiData {
    // assign this field to text, value, and value_text nests (defined at struct level)
    #[shrinkwrap(nest(id="text"), nest(id="value"), nest(id="value_text"))]
    // field attributes can also be injected, however there is no `class` filtering
    // as fields are always contained in `nest` struct variants and not `extra` or `wrapper`
    #[shrinkwrap_attr(attr(schemars(with = "String")), limit(nests("value")))]
    pub balance: f32,

    // assign this field to the text nest only, it will be excluded from `value` and `value->text`
    #[shrinkwrap(nest(id="text"))]
    #[shrinkwrap_attr(
        limit(nests("text")),
        attr(doc = "Timestamp in format: 'YYYY-MM-DD hh:MM:SS'")
    )]
    pub last_modified: u128,
}

/// Define the conversion from our `ApiData` struct into the generated `text` nest
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

/// Define the conversion from our `ApiData` struct into the generated `value` nest
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

/// Define the conversion from our **`value`** nest into the generated `value_text` sub-nest
impl TryTransformToNest<Option<ApiDataNestedValueText>> for MyTransform {
    type Data = ApiDataNestedValue; // must match generated struct name for the parent nest defined in `chain_from`
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

// !- Usage

pub fn main() -> Result<(), serde_json::Error> {
    println!("Starting example: {}", env!("CARGO_PKG_NAME"));

    let data = ApiData {
        balance: 82.231_21,
        last_modified: 1754443805,
    };

    let global_transform = MyTransform {};
    let transform_opts = MyTransformOpts {
        with_value: true,
        with_text: true,
    };

    let wrapped = data.try_to_wrapped_with(&global_transform, &transform_opts).unwrap();
    println!("Generated wrapper via transform: {wrapped:#?}");

    // note: shrinkwrap inline flag has renamded the wrapper and inlined all child data
    let wrapped_schema = schemars::schema_for_value!(wrapped).to_value();
    let wrapped_schema_json = serde_json::to_string_pretty(&wrapped_schema)?;
    println!("Wrapper schema: {wrapped_schema_json}");

    let wrapped_json = serde_json::to_string_pretty(&wrapped)?;
    println!("Serialized wrapper: {wrapped_json}");

    Ok(())
}
