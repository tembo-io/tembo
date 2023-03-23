// @generated automatically by Diesel CLI.

diesel::table! {
    extensions (id) {
        id -> Int8,
        name -> Nullable<Varchar>,
        updated_at -> Nullable<Timestamp>,
        created_at -> Nullable<Timestamp>,
        downloads -> Nullable<Int4>,
        description -> Nullable<Varchar>,
        homepage -> Nullable<Varchar>,
        documentation -> Nullable<Varchar>,
        repository -> Nullable<Varchar>,
    }
}

diesel::table! {
    versions (id) {
        id -> Int8,
        extension_id -> Nullable<Int4>,
        num -> Nullable<Varchar>,
        updated_at -> Nullable<Timestamp>,
        created_at -> Nullable<Timestamp>,
        downloads -> Nullable<Int4>,
        features -> Nullable<Jsonb>,
        yanked -> Nullable<Bool>,
        license -> Nullable<Varchar>,
        extension_size -> Nullable<Int4>,
        published_by -> Nullable<Int4>,
        checksum -> Nullable<Bpchar>,
        links -> Nullable<Varchar>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    extensions,
    versions,
);
