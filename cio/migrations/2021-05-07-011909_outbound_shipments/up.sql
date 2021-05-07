CREATE TABLE outbound_shipments (
    id SERIAL PRIMARY KEY,
    name VARCHAR NOT NULL,
    contents VARCHAR NOT NULL,
    street_1 VARCHAR NOT NULL,
    street_2 VARCHAR NOT NULL,
    city VARCHAR NOT NULL,
    state VARCHAR NOT NULL,
    zipcode VARCHAR NOT NULL,
    country VARCHAR NOT NULL,
    address_formatted VARCHAR NOT NULL,
    email VARCHAR NOT NULL,
    phone VARCHAR NOT NULL,
    status VARCHAR NOT NULL,
    carrier VARCHAR NOT NULL,
    tracking_number VARCHAR NOT NULL UNIQUE,
    tracking_link VARCHAR NOT NULL,
    oxide_tracking_link VARCHAR NOT NULL,
    tracking_status VARCHAR NOT NULL,
    label_link VARCHAR NOT NULL,
    reprint_label BOOLEAN NOT NULL DEFAULT 'f',
    resend_email_to_recipient BOOLEAN NOT NULL DEFAULT 'f',
    cost REAL NOT NULL DEFAULT 0,
    schedule_pickup BOOLEAN NOT NULL DEFAULT 'f',
    pickup_date DATE DEFAULT NULL,
    created_time TIMESTAMPTZ NOT NULL,
    shipped_time TIMESTAMPTZ DEFAULT NULL,
    delivered_time TIMESTAMPTZ DEFAULT NULL,
    eta TIMESTAMPTZ DEFAULT NULL,
    shippo_id VARCHAR NOT NULL,
    messages VARCHAR NOT NULL,
    notes VARCHAR NOT NULL,
    geocode_cache VARCHAR NOT NULL,
    airtable_record_id VARCHAR NOT NULL DEFAULT ''
)
