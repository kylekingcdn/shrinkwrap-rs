pub trait NestValueType {}

pub trait NestGroup {
    type Value: NestValueType;
}
