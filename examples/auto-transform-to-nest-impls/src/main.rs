use chrono::DateTime;
use schemars::JsonSchema;
use serde::Serialize;
use shrinkwrap::{Transform, TryBuildNestValue, TryToWrappedWith, Wrap};

// !- Transform

// Generated `try_transform_to_nest` use these fields to determine whether their optional nests
// are rendered.
//
// Field option names can be renamed with `derive_to_nest(options_field="..")`
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


// !- Auto-gen pre-req's

// Define a newtype wrapper for the target type
#[derive(Debug, Clone, JsonSchema, Serialize)]
pub struct NestedTextVariant(String);

impl shrinkwrap::NestValueType for NestedTextVariant {} // implement NestValueType on the value type


// Add one for the data representing our `value` nests as well
#[derive(Debug, Clone, JsonSchema, Serialize)]
pub struct NestedUsdValueVariant(f32);

impl shrinkwrap::NestValueType for NestedUsdValueVariant {}


// !- Data definition

#[derive(Debug, Copy, Clone, JsonSchema, Serialize, PartialEq, Eq)]
pub struct MyTimestampNewtype(u128);

/// Docs on origin `TestData`
#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Wrap)]
#[shrinkwrap(schema, inline, transform = MyTransform, all_optional, fallible(error = MyError))]
#[shrinkwrap(nest(id = "text", derive_to_nest(value = NestedTextVariant)))] // <---- replace `field_type=..` with `derive_to_nest(value = TypeWithImplForNestValueType)`
#[shrinkwrap(nest(id = "value", derive_to_nest(value = NestedUsdValueVariant)))] // use the usd value variant for value fields
#[shrinkwrap(
    nest(id = "value_text", derive_to_nest(value = NestedTextVariant), // reuse the text value again here, we just need to define
    field_name = "text", chain_from = "value"                          // `NestedUsdValueVariant` -> `TestedTextVariant` conversion once
))]                                                                    // and it can be implemented automatically (regardless of the nest)
pub struct ApiData {
    #[shrinkwrap(nest(id="text"), nest(id="value"), nest(id="value_text"))]
    #[shrinkwrap_attr(attr(schemars(with = "String")), limit(nests("value")))]
    pub balance: f32,

    // assign this field to the text nest only, it will be excluded from `value` and `value->text`
    #[shrinkwrap(nest(id="text"))]
    #[shrinkwrap_attr(
        limit(nests("text")),
        attr(doc = "Timestamp in format: 'YYYY-MM-DD hh:MM:SS'")
    )]
    pub last_modified: MyTimestampNewtype, // we use a newtype for the source now, as converting from u128 is ambiguous
}

// !- Auto-conversion handling

// text nests

/// Define how to convert each required source type into the destination `NestedTextVariant` type
impl TryBuildNestValue<f32, NestedTextVariant> for MyTransform {
    type Error = MyError;

    // simply define how to convert source type to dest type, regardless of the field/nest
    fn try_build_nest_value(&self, source: &f32, _options: &Self::Options) -> Result<NestedTextVariant, Self::Error> {
        Ok(NestedTextVariant(source.to_string()))
    }
}

/// Also add conversion for MyTimestampNewtype -> text to handle `last_modified`
impl TryBuildNestValue<MyTimestampNewtype, NestedTextVariant> for MyTransform {
    type Error = MyError;

    fn try_build_nest_value(&self, source: &MyTimestampNewtype, _options: &Self::Options) -> Result<NestedTextVariant, Self::Error> {
        let datetime = DateTime::from_timestamp(source.0 as i64, 0).unwrap();
        let datetime_txt = datetime.to_string();
        Ok(NestedTextVariant(datetime_txt))
    }
}

// usd value

impl TryBuildNestValue<f32, NestedUsdValueVariant> for MyTransform {
    type Error = MyError;
    fn try_build_nest_value(&self, source: &f32, _options: &Self::Options) -> Result<NestedUsdValueVariant, Self::Error> {
        let currency_price = 2.166281;
        let usd_value = source * currency_price;

        Ok(NestedUsdValueVariant(usd_value))
    }
}

// nested usd value -> text

// handled `value_text` nest population conversion

impl TryBuildNestValue<NestedUsdValueVariant, NestedTextVariant> for MyTransform {
    type Error = MyError;
    fn try_build_nest_value(&self, source: &NestedUsdValueVariant, _options: &Self::Options) -> Result<NestedTextVariant, Self::Error> {
        let usd_value_txt = format!("${:.2} USD", source.0);

        Ok(NestedTextVariant(usd_value_txt))
    }
}

// !- Usage

pub fn main() -> Result<(), serde_json::Error> {
    println!("Starting example: {}", env!("CARGO_PKG_NAME"));

    let data = ApiData {
        balance: 82.231_21,
        last_modified: MyTimestampNewtype(1777689504),
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

    // ..see below
    test_reusable_conversions();

    Ok(())
}

// This may seem more convoluted at the beginning, but transform conversions
// don't have to be redefined, e.g, this all works and uses the previous trait impls.
//
// (in fact, this is actually be called from main)

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Wrap)]
#[shrinkwrap(schema, inline, transform = MyTransform, all_optional, fallible(error = MyError))]
#[shrinkwrap(nest(id = "text", derive_to_nest(value = NestedTextVariant)))]
#[shrinkwrap(nest(id = "value", derive_to_nest(value = NestedUsdValueVariant)))]
#[shrinkwrap(nest(id = "value_text", derive_to_nest(value = NestedTextVariant), field_name = "text", chain_from = "value"))]
pub struct ReusedApiConvData {
    #[shrinkwrap(nest(id="text"), nest(id="value"), nest(id="value_text"))]
    #[shrinkwrap_attr(attr(schemars(with = "String")), limit(nests("value")))]
    pub credit: f32,

    #[shrinkwrap(nest(id="text"), nest(id="value"), nest(id="value_text"))]
    #[shrinkwrap_attr(attr(schemars(with = "String")), limit(nests("value")))]
    pub paid: f32,

    #[shrinkwrap(nest(id="text"), nest(id="value"), nest(id="value_text"))]
    #[shrinkwrap_attr(attr(schemars(with = "String")), limit(nests("value")))]
    pub total_fees: f32,

    #[shrinkwrap(nest(id="text"))]
    pub payment_sent_at: MyTimestampNewtype,

    #[shrinkwrap(nest(id="text"))]
    pub payment_received_at: MyTimestampNewtype,
}

fn test_reusable_conversions() {
    println!("\n-----\nDemonstrating auto transform impl reuse");

    let data = ReusedApiConvData {
        credit: 5.75,
        paid: 201.21,
        total_fees: 1.93,
        payment_sent_at: MyTimestampNewtype(1777229504),
        payment_received_at: MyTimestampNewtype(1777689504),
    };
    let global_transform = MyTransform {};
    let transform_opts = MyTransformOpts {
        with_value: true,
        with_text: true,
    };

    // all field conversions automatically wired up
    let wrapped = data.try_to_wrapped_with(&global_transform, &transform_opts).unwrap();

    let wrapped_json = serde_json::to_string_pretty(&wrapped).unwrap();
    println!("Serialized reusable conversion wrapper: {wrapped_json}");
}
