use chrono::{DateTime, Utc};
use serde::Serialize;
use shrinkwrap::{ToWrappedWith, Transform, TransformToNest, Wrap};
use uuid::Uuid;

// -- data definition

#[derive(Debug, Clone, Serialize, Wrap)]
#[shrinkwrap(transform = MyTransform)]
#[shrinkwrap(nest(id = "text", field_type = String))]
pub struct UserResponse {
    id: Uuid,
    username: String,
    #[shrinkwrap(nests("text"))]
    balance: i64, // balance in USD cents
    #[shrinkwrap(nests("text"))]
    last_login: DateTime<Utc>,
}
// The following structs have been created automatically:
// - `UserResponseWrapper`
// - `UserResponseExtra`
// - `UserResponseNestedText`

struct MyTransformOpts {
    // no run-time options required in this example
}
struct MyTransform {}
impl Transform for MyTransform {
    type Options = MyTransformOpts;
}

impl TransformToNest<UserResponseNestedText> for MyTransform {
    type Data = UserResponse;

    fn transform_to_nest(
        &self,
        data: &UserResponse,
        _options: &MyTransformOpts,
    ) -> UserResponseNestedText {
        UserResponseNestedText {
            balance: format!("${:.2} USD", data.balance as f32 / 100.0),
            last_login: data.last_login.format("%Y-%m-%d%l:%M%P").to_string(),
        }
    }
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transform = MyTransform {};
    let transform_opts = MyTransformOpts {};
    let data = UserResponse {
        id: Uuid::new_v4(),
        username: "johndoe".into(),
        balance: 27468,
        last_login: DateTime::from_timestamp(1754497944, 0).ok_or("Timestamp parse failed")?,
    };

    // generate the wrapper with your mapped data nested under 'extra'.
    let wrapped = data.to_wrapped_with(&transform, &transform_opts);
    println!("Generated wrapper struct debug output:\n{wrapped:#?}\n");
    println!(
        "Generated wrapper json: {}",
        serde_json::to_string_pretty(&wrapped)?
    );

    Ok(())
}
