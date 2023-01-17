use std::io::Result;
fn main() -> Result<()> {
    prost_build::Config::new()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".", "#[serde(rename_all = \"camelCase\")]")
        .field_attribute("merchant_data", "#[serde(default)]")
        .field_attribute("item_id", "#[serde(default)]")
        .compile_well_known_types()
        .extern_path(".google.protobuf.Timestamp", "::prost_wkt_types::Timestamp")
        .compile_protos(&["src/proto/api.proto"], &["src/proto"])?;
    Ok(())
}
