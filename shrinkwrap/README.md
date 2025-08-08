# `shrinkwrap`

Shrinkwrap makes it easy to render additional variations for a given set (or subset) of data.

A common use-case is providing human-readable variations of some coded format in an API response
(e.g. timestamps, numerics, currency balances, unit conversions).

## Overview

Consider the following struct which is used in an API response.

```rust
pub struct UserResponse {
    id: Uuid,
    username: String,
    balance: i64, // balance in USD cents
    last_login: DateTime<Utc>,
}
```

Suppose you wanted to provide API clients with balance and timestamps **ready for display**, e.g. `$123.45 USD`.

Shrinkwrap allows for single-call conversion ***from***

```json
{
    "id": "ab7aa843-ae3b-4120-b63e-e9d962120f9c",
    "username": "johndoe",
    "balance": 27468,
    "last_login": 1754497944
}
```

***To***

```json
{
    "id": "ab7aa843-ae3b-4120-b63e-e9d962120f9c",
    "username": "johndoe",
    "balance": 27468,
    "last_login": 1754497944,
    "extra": {
        "text": {
            "balance": "$274.68",
            "last_login": "Thu, 07 Aug 2025 02:14:44 GMT"
        }
    }
}
```

Shrinkwrap will provide conversion when you add `#[derive(Wrap)]` and a few required attributes to your
struct, plus a function to handle the actual conversion.

### Alternative solutions

Alternatively, you could...

- Newtype wrap your fields, providing a `Display` impl to override the original display format.

- Or, use `#[serde(with = ...)]` to provide custom serialization for the field.

If these suit your needs, great!

**But sometimes you need more control.**

## What `shrinkwrap` can do:

- Provide variants of data alongside the original, "untransformed" data
- Conditionally include/exclude variant groups at run-time
- Allow for handling conversions that are dependent on user-data (e.g. locale, user settings)
- Provide trivial support for chaining variations off of one another (using the output of one as input for another).
- Process conversions which have service dependencies

> [!NOTE]
> *Alternatively*, you *could* add fields for each variation directly into your main struct. e.g.
>
> - `id`
> - `username`
> - `balance`
> - `balance_text`
> - `balance_local`
> - `balance_local_text`
> - `last_login`
> - `last_login_text`
>
> However, **this clearly becomes unweildy:**
>
> Your structs become bloated with this extra baggage and conversions are being done all over the place.

## Usage

### Minimal example

To accomplish the example from the [Overview section](#overview):

1. Define your `transform` struct. This is the type that will be used to handle conversions.

   ```rust
   use shrinkwrap::Transform;

   struct MyTransformOpts {
       // no run-time options required in this example
   }
   struct MyTransform {}
   impl Transform for MyTransform {
       type Options = MyTransformOpts;
   }
   ```

2. Annotate your data struct, specifying the transform type from step 1, the nest definition, and
   fields to include in the nest

   ```rust
   use shrinkwrap::Wrap;

   #[derive(Debug, Clone, Serialize, Wrap)]
   #[shrinkwrap(transform = MyTransform)] // associate the wrapper/extra/nest conversion to your transform
   #[shrinkwrap(nest(id = "text", field_type = String))] // define a variant group (nest) for text repr
   pub struct UserResponse {                             //  + specify the return type for all fields within the nest
       id: Uuid,
       username: String,
       #[shrinkwrap(nests("text"))] // fields are opt-in and must be selected for each nest
       balance: i64,
       #[shrinkwrap(nests("text"))]
       last_login: DateTime<Utc>,
   }
   ```

3. Add the conversion impl

   ```rust
   use shrinkwrap::TransformToNest;

   impl TransformToNest<UserResponseNestedText> for MyTransform {
       type Data = UserResponse;

       fn transform_to_nest(&self, data: &UserResponse, _options: &MyTransformOpts) -> UserResponseNestedText {
           UserResponseNestedText {
               balance: format!("${:.2} USD", data.balance as f32 / 100.0),
               last_login: data.last_login.format("%Y-%m-%d%l:%M%P").to_string(),
           }
       }
   }
   ```

4. Use your derived impls + structs

   ```rust
   use shrinkwrap::ToWrappedWith;
   // -- snip -- //
   let transform = MyTransform {};
   let transform_opts = MyTransformOpts {};
   let data = UserResponse {
       id: Uuid::new_v4(),
       username: "johndoe".into(),
       balance: 27468,
       last_login: DateTime::from_timestamp(1754497944, 0).ok_or("Timestamp parse failed")?
    };

   // generate the wrapper with your mapped data nested under 'extra'.
   let wrapped = data.to_wrapped_with(&transform, &transform_opts);
   println!("Generated wrapper struct debug output:\n{wrapped:#?}\n");
   println!("Generated wrapper json: {}", serde_json::to_string_pretty(&wrapped)?);
   ```

---

The above example will output the following:

```
Generated wrapper struct debug output:
UserResponseWrapper {
    data: UserResponse {
        id: 2330f8a8-2f6b-4ed4-81a2-a4500db6ac33,
        username: "johndoe",
        balance: 27468,
        last_login: 2025-08-06T16:32:24Z,
    },
    extra: UserResponseExtra {
        text: UserResponseNestedText {
            balance: "$274.68 USD",
            last_login: "2025-08-06 4:32pm",
        },
    },
}
Generated wrapper json:
```
```json
{
  "id": "2330f8a8-2f6b-4ed4-81a2-a4500db6ac33",
  "username": "johndoe",
  "balance": 27468,
  "last_login": "2025-08-06T16:32:24Z",
  "extra": {
    "text": {
      "balance": "$274.68 USD",
      "last_login": "2025-08-06 4:32pm"
    }
  }
}
```

> [!NOTE]
> This example can be viewed and compiled in full at [`examples/readme`](https://github.com/kylekingcdn/shrinkwrap-rs/blob/main/examples/readme/src/main.rs)

## The `shrinkwrap` hierarchy

Shrinkwrap generates the following:

- A dedicated **`Nest`** struct for each defined variation set
- An **`Extra`** struct that contains all associated nests
- A **`Wrapper`** struct containing the original *data* struct, and the `Extra` struct.
  - **Note**: in the JSON above, the original *data* field of the wrapper has `#[serde(flatten)]`
  applied to it, giving the appearance of inlined data. This is done to reduce excessive nesting for consumers.

**Tree diagram:**

```
- wrapper
    - data (original data struct, which gets inlined into the wrapper)
      * field1
      * field2
    - extra
        - nest1:
          * field1
          * field2
        - nest2:
          * field1
          * field2
```

Variations are placed in a dedicated struct (the nests) to avoid polluting source data sets.
Each data set can support multiple nests, where each provides a distinct variation of a subset of the source data's fields.

### Nest chaining

Nests can be branched off one another, allowing for chained variations. An example of such flow:
1. `balance` in USD cents
2. to `balance` in local currency
3. and finally to `balance` in human readabable local currency

Continuing with the [first JSON example](#overview), where we are chaining:

- `balance` in USD cents
  - `balance` in USD in a human-readable format
  - `balance` in local currency
    - `balance` in local currency in a human-readable format

```json
{
    "id": "ab7aa843-ae3b-4120-b63e-e9d962120f9c",
    "username": "johndoe",
    "balance": 27468,
    "last_login": 1754532884,
    "extra": {
        "text": {
            "balance": "$274.68",
            "last_login": "Thu, 07 Aug 2025 02:14:44 GMT"
        },
        "local_value": {
            "balance": 37737,
            "extra": {
                "text": {
                    "balance": "$377.37 CAD"
                }
            }
        }
    }
}
```

## Attribute Reference

> [!CAUTION]
> **Work in progress - will be finished in the next patch**

## Links

- [crates.io](https://crates.io/crates/shrinkwrap/)
- [Documentation](https://docs.rs/shrinkwrap/)
- [Repository](https://github.com/kylekingcdn/shrinkwrap-rs)

## License

This project is licensed under the [MIT license](LICENSE.md).
