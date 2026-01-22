use std::time::Instant;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Weekday};
use serde::{Deserialize, Serialize};
use native_db::{db_type::Error, *};
use native_model::{native_model, Model};


fn main() {
    let target_date = 
    NaiveDate::from_ymd_opt(2026, 12, 31)
    .expect("Invalid date provided");


    let first_day = NaiveDate::from_ymd_opt(2026, 1, 15).expect("Invalid date");

    // 2. Initialize the Job struct
    let my_job = Job {
        id: 101,
        name: String::from("Senior Technician"),
        basic_pay: 2550, // $25.50 in cents
        base_hours: Some(38),
        shift_pattern: Some(ShiftPattern::SixOnTwoOff),
        overtime_multiplier: Some(1.5),
        saturday_multiplier: Some(1.5),
        sunday_multiplier: Some(2.0),
        bank_holiday_multiplier: Some(2.5),
        christmass_day_multiplier: Some(3.0),
        unsociable_hours_multiplier: Some(1.2),
        
        // Pattern starts on March 16th, 2026
        first_day: NaiveDate::from_ymd_opt(2026, 3, 16),
        
        // Fixed 8:30 AM start
        fixed_start_time: NaiveTime::from_hms_opt(8, 30, 0),
        
        // Precise 8 hour, 15 minute, 30 second shift
        fixed_shift_duration: Some(
            Duration::hours(8) + 
            Duration::minutes(15) + 
            Duration::seconds(30)
        ),
    };
    let start_date = NaiveDate::from_ymd_opt(2026, 01, 18).expect("");
    let end_date = NaiveDate::from_ymd_opt(2026, 01, 24).expect("");
    print_shifts(my_job, start_date, end_date);

    // let feb = my_job.get_shifts_for_month(2, 2026);

    // for day in feb {
    //      let status_text = match day.status {
    //         ShiftStatus::ON => "WORK",
    //         _ => "OFF"
    //     };
    //     println!("{}: {} (Cycle Day: {}, Week Day: {})", day.date, status_text, day.day_in_cycle, day.date.weekday());
    }

fn print_shifts(my_job: Job,start_date: NaiveDate, target_date: NaiveDate) {
    let start = Instant::now();
    let schedule = my_job.get_scheduled_shifts_for_period(start_date, target_date);

    let duration = start.elapsed();

    if(schedule.is_empty()) {
        println!("No shifts found from {:?} to {:?}", start_date.to_string().replace("-", "/"), target_date.to_string().replace("-", "/"));
        return
    }
    for day in schedule {
        let status_text = match day.status {
            ShiftStatus::ON => "WORK",
            _ => "OFF"
        };

        println!("{}: {} (Week Day: {})", day.date, status_text, day.date.weekday());
    }
    println!("elapsed: {:?}", duration)
}

// If sunday, add one day to get the week commencing. // If Sunday to Saturday rota.
struct TaxWeek {
    week_commencing: u8,
    financial_year: String, // e.g. 2025/2026, 2026/2027
    tax_week_start: Option<TaxWeekStart>,
    week_start_date: NaiveDate,
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum TaxWeekStart {
    Sunday,
    Monday
}
impl TaxWeek {
    fn new(date: NaiveDate, tax_week_start: TaxWeekStart) -> TaxWeek {
        let fixed_date = if matches!(tax_week_start, TaxWeekStart::Sunday) && date.weekday() == Weekday::Sun {
            date + TimeDelta::days(1)
        } else { date };

        let week = TaxWeek::get_week_commnencing(fixed_date);
        let financial_year = TaxWeek::get_financial_year(fixed_date);

        let week_start_date = if matches!(tax_week_start, TaxWeekStart::Sunday) {
            date - TimeDelta::days(date.weekday().num_days_from_sunday() as i64)
        } else {
            date - TimeDelta::days(date.weekday().num_days_from_monday() as i64)
        };

        TaxWeek {
             week_commencing: week, 
             financial_year: financial_year, 
             tax_week_start: Some(tax_week_start),
             week_start_date: week_start_date
            }
    }
    fn get_week_commnencing(date: NaiveDate) -> u8 {
        let cycle_start_of_financial_year = TaxWeek::get_year_cycle_of_financial_year(date);G
        let financial_year_start_date = NaiveDate::from_ymd_opt(cycle_start_of_financial_year, 4, 6).expect("invalid date format");

        let days_elapsed = (date - financial_year_start_date).num_days();
        
        ((days_elapsed / 7)+ 1) as u8
    }
    // Returns a year when a given financial year started.
    fn get_year_cycle_of_financial_year(date: NaiveDate) -> i32 {
        if date.month() < 4 || (date.month() == 4 && date.day() < 6) {
            date.year() - 1
        } else {
            date.year()
        }
    }
    fn get_financial_year(date: NaiveDate) -> String {
        if date.month() < 4 || (date.month() == 4 && date.day() < 6) {
            format!("{}/{}", date.year() - 1, date.year())
        } else {
            format!("{}/{}", date.year(), date.year() + 1)
        }
    }
}


enum ShiftPattern {
    SixOnTwoOff,
    FourOnFourOff(AveragePatternMatch), // 
    Custom(Vec<Weekday>),
}
// If not paid on average,
// Calculation should be proceeded by the weekly/monthly total amount of hours worked
// If paid on average, use: (total_hours_in_a_week / 8 (as 8 days is a cycle) * 7)
struct AveragePatternMatch {
    is_paid_on_average: bool,

}
impl ShiftPattern {
    fn get_base_days_on(&self, date: NaiveDate, job: &Job) -> i32 {
        match self {
            ShiftPattern::SixOnTwoOff => 5, // It is always 5 in this pattern
            ShiftPattern::FourOnFourOff(_) => { // Shift vary, depending on the week
                // Get the first day of the tax week,
                // Find the last day (+6)
                // Get the days working (only)

                let tax_week = TaxWeek::new(date,job.get_tax_week_start());
                let first_day = tax_week.week_start_date;
                let last_day = first_day + TimeDelta::days(6);
                
                let schedule  = job.get_scheduled_shifts_for_period(first_day, last_day)
                .into_iter()
                .filter(|shift|{
                    shift.status == ShiftStatus::ON
                })
                .count();

                schedule as i32
            },
            ShiftPattern::Custom(days) => {
                days.into_iter().count() as i32
            }
            _ => { 0 }
        }
    }
}

impl ShiftPattern {
    fn get_custom_days(&self) -> Option<&Vec<Weekday>> {
        match self {
            ShiftPattern::Custom(v) => Some(v),
            _ => None,
        }
    }
}
struct Job {
    id: i32,
    name: String,
    basic_pay: i32,
    base_hours: Option<u32>, // Daily, if None, don't calculate the overtime.
    shift_pattern: Option<ShiftPattern>,
    overtime_multiplier: Option<f32>,
    saturday_multiplier: Option<f32>,
    sunday_multiplier: Option<f32>,
    bank_holiday_multiplier: Option<f32>,
    christmass_day_multiplier: Option<f32>,
    unsociable_hours_multiplier: Option<f32>,
    // The day marked as the beginning of the shift-pattern.
    first_day: Option<NaiveDate>,
    fixed_start_time: Option<NaiveTime>,
    fixed_shift_duration: Option<Duration>,
    tax_week_start: Option<TaxWeekStart>,
}
impl Job {
    fn get_tax_week_start(&self) -> TaxWeekStart {
        self.tax_week_start.unwrap_or(TaxWeekStart::Sunday)
    }
    fn get_shifts_for_period_of(&self,  start_date: NaiveDate, end_date: NaiveDate) -> Vec<Shift> {
        let shifts: Vec<Shift> = Vec::new();
        // TODO: Fetch shifts data from the database for a given period
        // Note: First try to get them from the global context of dioxus, as it is
        // unnecessary to load them multiple times.
        Vec::new()
    }
    fn get_scheduled_shifts_for_period(&self, start_date: NaiveDate, end_date: NaiveDate) -> Vec<ScheduledShift> {
        let shifts = self.calculate_scheduled_shifts_up_to(end_date);

        shifts
        .into_iter()
        .filter(|day| day.date >= start_date && day.date <= end_date)
        .collect()
    }
    fn get_scheduled_shifts_for_month(&self, target_month: u32, target_year: i32) -> Vec<ScheduledShift> {
        let month = if(target_month + 1) == 13 { 12 } else { target_month } + 1;
        let target_date = NaiveDate::from_ymd_opt(target_year, month, 1).expect("Couldn't find the date specified");

        let schedule = self.calculate_scheduled_shifts_up_to(target_date);

        let dates = schedule
        .into_iter()
        .filter(|date| date.date.month() == target_month && date.date.year() == target_year)
        .collect();

        dates
    }
    fn calculate_scheduled_shifts_up_to(&self, target_date: NaiveDate) -> Vec<ScheduledShift> {
        return match self.shift_pattern {
            Some(ShiftPattern::SixOnTwoOff) => {
                self.calculate_scheduled_shifts_for_six_on_two_off(target_date)
            }
            Some(ShiftPattern::FourOnFourOff(_)) => {
                self.calculate_scheduled_shifts_for_four_on_four_off(target_date)
            }
            Some(ShiftPattern::Custom(_)) => {
                self.calculate_scheduled_shifts_for_custom(target_date)
            }
            _ => {
                Vec::new()
            }
        };
    }
    // Make sure to check if any days are selected!
    fn calculate_scheduled_shifts_for_custom(&self, target_date: NaiveDate) -> Vec<ScheduledShift> {
        let Some(first_day) = self.first_day else {
            return Vec::new()
        };
        let Some(shift_pattern) = self.shift_pattern.as_ref() else {
            return Vec::new()
        };

        let mut schedule: Vec<ScheduledShift> = Vec::new();

        let mut current_day = ScheduledShift {
            job_id: self.id,
            date: first_day,
            status: ShiftStatus::ON,
            day_in_cycle: 0,
        };
        let days_working = shift_pattern.get_custom_days().expect("Error occurred while looking for Custom Weekdays.");

        while current_day.date <= target_date {
            schedule.push(current_day);

            let next_date = current_day.date.succ_opt().unwrap();
            let next_status = if !days_working.contains(&next_date.weekday()) { ShiftStatus::OFF} else { ShiftStatus::ON }; 
            current_day = ScheduledShift {
                job_id: self.id,
                date: next_date,
                status: next_status,
                day_in_cycle: 0
            };
        }

        schedule
    }

    // Ask for the first day in the UI
    fn calculate_scheduled_shifts_for_four_on_four_off(&self, target_date: NaiveDate) -> Vec<ScheduledShift> {
        let Some(first_day) = self.first_day else { 
            return Vec::new()
        };

        let mut schedule: Vec<ScheduledShift> = Vec::new();

        let mut current_day = ScheduledShift {
            job_id: self.id,
            date: first_day,
            status: ShiftStatus::ON,
            day_in_cycle: 1,
        };

        while current_day.date <= target_date {
            schedule.push(current_day);

            let next_date = current_day.date.succ_opt().unwrap();
            let next_day_in_cycle = if(current_day.day_in_cycle + 1) > 8 {
                1
            } else { current_day.day_in_cycle + 1 };

            let next_status = if next_day_in_cycle > 4 {
                ShiftStatus::OFF
            } else { ShiftStatus::ON };

            current_day = ScheduledShift {
                job_id: self.id,
                date: next_date,
                status: next_status,
                day_in_cycle: next_day_in_cycle
            }
        }

        schedule
    }

    // Ask for the first day in the UI
    fn calculate_scheduled_shifts_for_six_on_two_off(&self, target_date: NaiveDate) -> Vec<ScheduledShift> {
         let Some(first_day) = self.first_day else { 
            return Vec::new()
        };

        let mut schedule: Vec<ScheduledShift> = Vec::new();

        let mut current_day = ScheduledShift {
            job_id: self.id,
            date: first_day,
            status: ShiftStatus::ON,
            day_in_cycle: 1,
        };

        let mut current_block_start_weekday = first_day.weekday();

        while current_day.date <= target_date {
            schedule.push(current_day);

            let next_date = current_day.date.succ_opt().unwrap();
            let next_weekday = next_date.weekday();

            let cycle_limit = if current_block_start_weekday == Weekday::Sat { 9} else { 8 };

            let next_day_in_cycle = if(current_day.day_in_cycle + 1) > cycle_limit {
                1
            } else {
                current_day.day_in_cycle + 1
            };
            if next_day_in_cycle == 1 { current_block_start_weekday = next_weekday }; 

            let next_status = if current_block_start_weekday == Weekday::Mon {
                if next_day_in_cycle >= 6 {
                    ShiftStatus::OFF
                } else {
                    ShiftStatus::ON
                }
            } else {
                if next_day_in_cycle >= 7 {
                    ShiftStatus::OFF
                } else {
                    ShiftStatus::ON
                }
            };

            // match current_block_start_weekday {
            //     Weekday::Mon => {
                    

            //     },
            //     Weekday::Sat => {

            //     }
            //     _ => {

            //     }
            // };
            // if 1st day is tuesday, wednesday or thursday, 
            // keep going with the schedule until the sixth day.

            // if the first day is Monday - Saturday, Sunday and Monday are days OFF!
            // if the first day is Saturday - Friday, Saturday and Sunday are days OFF!

            current_day = ScheduledShift {
                job_id: self.id,
                date: next_date,
                status: next_status,
                day_in_cycle: next_day_in_cycle
            }

        }
        schedule
    }

    fn is_working_on(&self, target_date: NaiveDate, schedule: &[ScheduledShift]) -> bool {
        schedule
        .iter()
        .find(|day| day.date == target_date)
        .map(|day| day.status == ShiftStatus::ON)
        .unwrap_or(false)
    }
}
#[derive(Clone, Copy)]
struct ScheduledShift {
    job_id: i32,
    date: NaiveDate,
    status: ShiftStatus,
    day_in_cycle: i32
}
#[derive(Clone, Copy, PartialEq)]
enum ShiftStatus {
    OFF,
    ON
}
struct Shift {
    id: i32,
    job_id: i32,
    date: NaiveDate,
    shift_type: ShiftType,
    start: NaiveDateTime,
    finish: NaiveDateTime,

}

// saved in the database
struct Deduction {
    id: i32,
    shift_id: Option<i32>, // As it may not be assigned directly to a shift, it may be scheduled.
    name: String,
    description: String,
    amount: u32,
    date: Option<NaiveDate>
}

// Fetch specific deductions from the native database
// TODO later try to fetch them from global context of dioxus first!
impl Deduction {
    fn get_deductions_for(date: NaiveDate) -> Vec<Deduction> {
        todo!()
    }
}

struct ShiftRecord {
    shift: Shift,
    deductions: Option<Vec<Deduction>>,
    payment: Option<ShiftPayment>,
}

impl Shift {
    fn get_pretty_time_worked(&self) -> String {
        let time = self.get_time_worked();

        let h = time.num_hours();
        let m = time.num_minutes() % 60;
        let s = time.num_seconds() % 60;

        format!("{}h, {}m, {}s", h, m, s)
    }

    fn get_time_worked(&self) -> TimeDelta {
        self.finish.signed_duration_since(self.start)

    }
}
enum ShiftType {
    Sick,
    Holiday,
    PaidLeave,
    ExtraShift
}


// Shift Pay is generated automatically, no need to save in the database!
struct ShiftPayment {
    shift_id: i32,
    job_id: i32,

    net_pay: u32,
    gross_pay: Option<u32>,
    deductions: Option<u32>, // Only applicable, if set up by the user
    
    payment: Option<ShiftPaymentType>,
}

impl ShiftPayment {

    /*
    
    Get total payment for a given day
    - Check the day to see what value to give

     */
    fn get_pay_for(shift: &Shift, job: &Job) -> ShiftPayment {
        // check whether the user has scheduled their shifts

        let shift_payment = ShiftPayment { 
            shift_id: shift.id,
            job_id: job.id,

            net_pay: 32,
            gross_pay: None, // Add in the future, after introducing taxes TODO
            deductions: None, // Add later after adding deductions TODO

            payment: Some(ShiftPaymentType::Basic(3200)) // todo change it
        };

        shift_payment
    }

    fn new_for(&self, shift: &Shift, job: &Job) -> ShiftPayment {
        ShiftPayment::get_pay_for(shift, job);
        
        todo!()
    }
    fn get_pay_for_period_of(start_date: NaiveDate, finish_date: NaiveDate, job: &Job) -> Vec<ShiftPayment> {
        todo!()
    }
}
enum ShiftPaymentType {
    Basic(u32),
    Sunday(u32),
    Saturday(u32),
    Overtime(u32),
    BankHoliday(u32),
    Christmass(u32),
    Unsociable(u32),

    // For example a bonus
    Custom(CustomShiftPaymentType),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum CustomShiftPaymentReoccurement {
    No,
    // weekday on which it reoccurs!
    Yes(Weekday)
}
#[native_model(id = 2, version = 1)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[native_db]
struct CustomShiftPaymentType {
    #[primary_key]
    id: i32,
    #[secondary_key(unique)]
    job_id: i32,

    shift_id: i32,

    name: String,
    is_taxable: Option<bool>,
    day: Option<NaiveDate>,
    // The user can chose multiplier and additional amount
    // of money for a custom shift.
    multiplier: Option<f64>,
    amount: Option<u32>,
    reoccurement: Option<CustomShiftPaymentReoccurement>,
}


/*

ShiftPayment is generated automatically >
ShiftPaymentType is generated automatically based on the day >
> Therefore CustomShiftPaymentType is the only one that has to be loaded from the database, as...
> It is being saved by the user.

*/


/*



*/
impl CustomShiftPaymentType {
    fn get_reoccuring_payments(db: &Database, date: NaiveDate) -> Result<Vec<CustomShiftPaymentType>, Error> {
        let r_txn = db.r_transaction()?;
        // Load from database
        let payments: Vec<Result<CustomShiftPaymentType, Error>> = r_txn
        .scan()
        .secondary::<CustomShiftPaymentType>(CustomShiftPaymentTypeKey::job_id)?
        .all()?
        .map(|data| data)
        .collect();

        let filtered_payments: Vec<CustomShiftPaymentType> = payments
        .into_iter()
        .filter_map(|res| res.ok())
        .filter(|payment| {
        // 1. Try to get the day directly
        // 2. If day is None, check reoccurement
        let payment_day = if let Some(day) = payment.day {
            Some(day)
        } else if let Some(reoccurement) = &payment.reoccurement {
            match reoccurement {
                CustomShiftPaymentReoccurement::Yes(weekday) => {
                    if date.weekday() == *weekday { Some(date) } else { None }
                },
                CustomShiftPaymentReoccurement::No => None,
            }
        } else {
            None
        };

        // 3. Compare the extracted day to your target date's weekday
        // Replace `date.weekday()` with however you get the weekday from your reference date
        payment_day == Some(date)
    })
        .collect();

        Ok(filtered_payments)
    }
}

/*

struct Shift {
    id: i32,
    job_id: i32,
    date: NaiveDate,
    shift_type: ShiftType,
    start: NaiveDateTime,
    finish: NaiveDateTime,
}

*/

// Todo: Add bonuses


/*
struct Job {
    id: i32,
    name: String,
    basic_pay: i32,
    base_hours: Option<u32>,
    shift_pattern: Option<ShiftPattern>,
    overtime_multiplier: Option<f32>,
    saturday_multiplier: Option<f32>,
    sunday_multiplier: Option<f32>,
    bank_holiday_multiplier: Option<f32>,
    christmass_day_multiplier: Option<f32>,
    unsociable_hours_multiplier: Option<f32>,
    // The day marked as the beginning of the shift-pattern.
    first_day: Option<NaiveDate>,
    fixed_start_time: Option<NaiveTime>,
    fixed_shift_duration: Option<Duration>,

}
 */
