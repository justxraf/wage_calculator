# Wage Calculator

A Rust-based wage calculation system with shift scheduling, tax calculations, and deduction management for UK employees.

## Project Overview

This application calculates wages, manages work shifts, handles deductions, and computes UK tax obligations including region-specific calculations for Scotland, England, Wales, and Northern Ireland.

## Core Features

### 1. Job Management
- Store multiple jobs with configurable pay rates and multipliers
- Support for various shift patterns:
  - Six-on-two-off
  - Four-on-four-off
  - Custom weekly patterns
- Configurable pay multipliers:
  - Overtime (default 1.5x)
  - Saturday shifts (default 1.5x)
  - Sunday shifts (default 2.0x)
  - Bank holidays (default 2.5x)
  - Christmas Day (default 3.0x)
  - Unsociable hours (default 1.2x)

### 2. Shift Tracking
- Record individual shifts with start/end times
- Support for different shift types:
  - Regular shifts
  - Extra shifts
  - Sick leave
  - Holiday
  - Paid leave
- Automatic shift scheduling based on job patterns
- Date-based querying for shift history

### 3. Deduction System
- Pre-tax and post-tax deductions
- Multiple scheduling options:
  - One-time deductions
  - Weekly recurring (specific weekdays)
  - Monthly recurring (specific days)
  - Custom date patterns
- Automatic application based on date rules

### 4. Tax Calculations
- Scottish Income Tax (7 bands, 19%-48%)
- Rest of UK Income Tax (3 bands, 20%-45%)
- National Insurance contributions
- Personal allowance with high-income taper
- 2026/27 tax year rates implemented

### 5. Payment Processing
- Shift-based payment calculations
- Gross and net pay computation
- Tax and NI predictions
- Custom payment types with multipliers

## Technical Architecture

### Database
- **Storage**: `native_db` (embedded database)
- **Models**: `native_model` for schema definitions
- **Entities**: Job, Shift, Deduction, CustomShiftPaymentType

### Key Data Structures

#### Job (src/main.rs:240-258)
Primary entity containing employment details, pay rates, and shift patterns.

#### Shift (src/main.rs:552-567)
Individual work shift records with date indexing for efficient querying.

#### Deduction (src/main.rs:573-590)
Configurable deductions with scheduling logic.

#### TaxSummary (src/main.rs:904-1087)
Tax calculation engine with region-specific logic.

### ID Management
Centralized ID generation system using atomic counters (src/main.rs:1184-1288) ensures unique IDs across all entity types.

## Dependencies

```toml
chrono = "0.4.43"          # Date/time handling
native_db = "0.8.2"        # Embedded database
native_model = "0.4.20"    # Schema definitions
once_cell = "1.21.3"       # Lazy statics
serde = "1.0.228"          # Serialization
```

## Usage Patterns

### Creating a Job
```rust
let job = Job::new(id_gen, name, basic_pay)
    .with_shift_pattern(ShiftPattern::SixOnTwoOff)
    .with_base_hours(38)
    .with_overtime_multiplier(1.5)
    .with_first_day(start_date);
```

### Querying Shifts
```rust
// Get shifts for a date range
let shifts = job.get_shifts_for_period_of(start_date, end_date, &db)?;

// Get scheduled shifts
let schedule = job.get_scheduled_shifts_for_period(start_date, target_date);
```

### Tax Calculations
```rust
let tax_summary = TaxSummary::new(&shift_payment, UKRegion::Scotland);
let tax = tax_summary.get_tax_prediction();
let ni = tax_summary.get_national_insurance_prediction();
let net = tax_summary.get_gross_after_deductions();
```

## Implementation Status

### Complete
- Job creation and management
- Shift pattern calculations (all 3 types)
- Tax calculation engine (all UK regions)
- National Insurance calculations
- Deduction scheduling logic
- ID generation system
- Database schema

### In Progress (TODOs)
- ShiftPayment generation (src/main.rs:848-862)
- Payment summaries (src/main.rs:870-874)
- Custom payment type integration
- Full shift-to-payment pipeline

## Date Handling

- All monetary amounts stored in **pence** (u32)
- Dates use `NaiveDate` from chrono
- Tax weeks calculated from UK tax year (April 6)
- Support for both Sunday and Monday week starts

## Tax Year Information

The system uses UK tax year 2026/27 rates:
- Tax year runs from April 6, 2026 to April 5, 2027
- Personal allowance: £12,570
- Personal allowance tapers for income over £100,000

## Known Issues

1. Edition warning in Cargo.toml (line 4: edition = "2024" should be "2021")
2. Several TODO items in payment generation logic
3. Error handling needs improvement (several `.unwrap()` calls)

## Development Notes

- All calculations use pence to avoid floating-point errors
- Database queries use secondary key indices for performance
- Shift scheduling generates full calendars efficiently
- Builder pattern used for Job creation flexibility
