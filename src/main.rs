use std::{collections::{HashMap, HashSet}, f32::consts::PI, sync::{Arc, atomic::{AtomicI32, Ordering}}, time::Instant};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Weekday};
use serde::{Deserialize, Serialize, de};
use native_db::{db_type::Error, *};
use native_model::{Error, Model, native_model};
use once_cell::sync::Lazy;

static BANK_HOLIDAYS: Lazy<BankHolidayChecker> = Lazy::new(|| {
    BankHolidayChecker::new(vec![
        BankHoliday::new(NaiveDate::from_ymd_opt(2026, 12, 25).unwrap(), "Christmas".to_string()),
        BankHoliday::new(NaiveDate::from_ymd_opt(2026, 12, 26).unwrap(), "Boxing Day".to_string()),
    ])
});


static MODELS: Lazy<Models> = Lazy::new(|| {
    let mut models = Models::new();
    models.define::<Job>().unwrap();
    models.define::<Deduction>().unwrap();
    models.define::<Shift>().unwrap();
    models.define::<CustomShiftPaymentType>().unwrap();
    models
});
fn main() -> Result<(), db_type::Error> {
    let mut db = Builder::new().create_in_memory(&MODELS)?;
    let id_gen = Arc::new(IdGenerator::new(&db).unwrap());
    // Load the jobs! There is not much data that goes in them, so having them in the cache
    // Is a better option, than loading "on-demand."
    let jobs = Arc::new(Job::load_all(&db).unwrap());


    // Example 1: Create a new Job
    let job_id = id_gen.next_job_id();
    let my_job = Job {
        id: job_id,
        name: String::from("Senior Technician"),
        basic_pay: 2550,
        base_pay_period_hours: Some(38),
        shift_pattern: Some(ShiftPattern::SixOnTwoOff),
        overtime_multiplier: Some(1.5),
        saturday_multiplier: Some(1.5),
        sunday_multiplier: Some(2.0),
        bank_holiday_multiplier: Some(2.5),
        christmass_day_multiplier: Some(3.0),
        unsociable_hours_time_window: None,
        unsociable_hours_multiplier: Some(1.2),
        first_day: NaiveDate::from_ymd_opt(2026, 3, 16),
        fixed_start_time: NaiveTime::from_hms_opt(8, 30, 0),
        fixed_shift_duration: Some(
            Duration::hours(8) + Duration::minutes(15) + Duration::seconds(30)
        ),
        tax_week_start: Some(TaxWeekStart::Sunday),
    };

    println!("begins with: {:?}, {:?} per second, converted back is {:?}",my_job.basic_pay, my_job.get_basic_hours_base_rate_per_second(),
my_job.get_basic_hours_base_rate_per_second() * 3600.0);


    // Example 2: Create a new Shift using generic syntax
    let shift_id = id_gen.next_id::<Shift>();
    let shift = Shift::new(
        shift_id,
        job_id,
        NaiveDate::from_ymd_opt(2026, 1, 20).unwrap(),
        ShiftType::ExtraShift,
        NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2026, 1, 20).unwrap(),
            NaiveTime::from_hms_opt(8, 30, 0).unwrap()
        ),
        NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2026, 1, 20).unwrap(),
            NaiveTime::from_hms_opt(16, 45, 30).unwrap()
        ),
    );

    // Example 3: Create a Deduction
    let deduction_id = id_gen.next_deduction_id();
    let deduction = Deduction {
        id: deduction_id,
        job_id,
        shift_id,
        name: String::from("Pension Contribution"),
        description: Some(String::from("Monthly pension deduction")),
        amount: 100_00, // £100.00 in pence
        is_pre_tax: true,
        schedule: ReocurrementSchedule::Monthly {
            day_of_month: vec![1],
            start_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end_date: None,
        },
    };

    // Example 4: Pass id_gen to functions
    create_shift_with_deductions(&db, &id_gen, job_id);

    Ok(())
}

// Helper function that uses id_gen
fn create_shift_with_deductions(db: &Database, id_gen: &IdGenerator, job_id: i32) {
    let shift_id = id_gen.next_shift_id();
    let deduction_id = id_gen.next_deduction_id();
    
    // Create entities...
    println!("Created shift {} with deduction {}", shift_id, deduction_id);
}


fn print_shifts(my_job: Job,start_date: NaiveDate, target_date: NaiveDate) {
    let start = Instant::now();
    let schedule = my_job.get_scheduled_shifts_for_period(start_date, target_date);

    let duration = start.elapsed();

    if schedule.is_empty() {
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
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
struct TaxWeek {
    week_commencing: u8,
    financial_year: String, // e.g. 2025/2026, 2026/2027
    tax_week_start: Option<TaxWeekStart>,
    week_start_date: NaiveDate,
}

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
enum TaxWeekStart {
    Sunday,
    Monday
}
impl TaxWeek {
    fn new(date: NaiveDate, tax_week_start: TaxWeekStart) -> TaxWeek {
        let fixed_date = if matches!(tax_week_start, TaxWeekStart::Sunday) 
            && date.weekday() == Weekday::Sun {
            date.checked_add_signed(TimeDelta::days(1))
                .unwrap_or(date) // fallback to original date
        } else { date };

        let week = TaxWeek::get_week_commnencing(fixed_date);
        let financial_year = TaxWeek::get_financial_year(fixed_date);

        let week_start_date = if matches!(tax_week_start, TaxWeekStart::Sunday) {
            date
            .checked_sub_signed(
                TimeDelta::days(date.weekday().num_days_from_sunday() as i64)
            )
            .unwrap() // todo -- make sure to handle error cases.
        } else {
            date
            .checked_sub_signed(
                TimeDelta::days(date.weekday().num_days_from_monday() as i64)
            )
            .unwrap() // todo -- handle error case
        };

        TaxWeek {
             week_commencing: week, 
             financial_year: financial_year, 
             tax_week_start: Some(tax_week_start),
             week_start_date: week_start_date
            }
    }
    fn get_week_commnencing(date: NaiveDate) -> u8 {
        let cycle_start_of_financial_year = TaxWeek::get_year_cycle_of_financial_year(date);
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

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
enum ShiftPattern {
    SixOnTwoOff,
    FourOnFourOff(AveragePatternMatch), // 
    Custom(Vec<Weekday>),
}
// If not paid on average,
// Calculation should be proceeded by the weekly/monthly total amount of hours worked
// If paid on average, use: (total_hours_in_a_week / 8 (as 8 days is a cycle) * 7)
#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
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


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[native_model(id = 4, version = 1)]
#[native_db]
struct Job {
    #[primary_key]
    id: i32,
    name: String,
    basic_pay: i32,
    base_pay_period_hours: Option<u32>, // Daily, if None, don't calculate the overtime.
    shift_pattern: Option<ShiftPattern>,
    // The day marked as the beginning of the shift-pattern.
    first_day: Option<NaiveDate>,
    fixed_start_time: Option<NaiveTime>,
    fixed_shift_duration: Option<Duration>,
    tax_week_start: Option<TaxWeekStart>,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[native_model(id = 8, version = 1)]
#[native_db]
struct SalaryMultiplier {
    #[primary_key]
    id: i32,
    #[secondary_key]
    job_id: i32,
    behavior: MultiplierBehavior,
    // Allow the user to choose between "Always apply" (e.g. night-shift), 
    // "Low-Priority", "Medium-Priority" (e.g. Saturday, Sunday), "High-Priority" (e.g. Bank Holidays)
    priority: MultiplierPriority,

    name: String,
    description: Option<String>,
    schedule: ReocurrementSchedule,
    multiplier: f32,
    time_window: Option<TimeWindow>,
}

// Allow to select a given time window for a shift.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct TimeWindow {
    start: NaiveTime,
    end: NaiveTime
}
impl TimeWindow {
    fn calculate_time_overlap_seconds(
        &self,
        shift_start: NaiveDateTime,
        shift_end: NaiveDateTime,
    ) -> i64 {
        let window_start = self.start;
        let window_end = self.end;

        let window_crosses_midnight = window_end < window_start;
        
        let mut total_seconds = 0i64;
        let mut current = shift_start;
        
        // Process each day the shift spans
        while current < shift_end {
            let day_end = current.date().and_hms_opt(23, 59, 59).unwrap().min(shift_end);
            let day_start = current.time();
            let day_end_time = day_end.time();
            
            if window_crosses_midnight {
                // Evening period (window_start to midnight)
                if day_start < NaiveTime::from_hms_opt(23, 59, 59).unwrap() {
                    let period_start = day_start.max(window_start);
                    let period_end = day_end_time;
                    
                    if period_end >= window_start {
                        let overlap_start = period_start.max(window_start);
                        let overlap_end = period_end;
                        if overlap_end > overlap_start {
                            total_seconds += (overlap_end - overlap_start).num_seconds();
                        }
                    }
                }
                
                // Morning period (midnight to window_end)
                let morning_start = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
                if day_end_time > morning_start && window_end > morning_start {
                    let overlap_start = day_start.max(morning_start);
                    let overlap_end = day_end_time.min(window_end);
                    if overlap_end > overlap_start {
                        total_seconds += (overlap_end - overlap_start).num_seconds();
                    }
                }
            } else {
                // Simple case: window doesn't cross midnight
                let overlap_start = day_start.max(window_start);
                let overlap_end = day_end_time.min(window_end);
                
                if overlap_end > overlap_start {
                    total_seconds += (overlap_end - overlap_start).num_seconds();
                }
            }
            
            // Move to next day
            current = current.date().succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap();
            if current > shift_end {
                break;
            }
        }
        
        total_seconds
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum MultiplierPriority {
    AlwaysApply,
    Low,
    Medium,
    High,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum MultiplierBehavior {
    Compound,
    HighestOnly,
}
struct MultiplierResult {
    taken_amount: f32,
    final_amount: f32,
    multipliers: Vec<&SalaryMultiplier>,
    top_multiplier: Option<SalaryMultiplier>, // If the multiplier is marked as the highest only. 
}
// TODO - Add database support (save the value in the database!)
impl SalaryMultiplier {
    fn load_all(job_ids: Vec<i32>, db: &Database) -> Result<HashMap<i32, Vec<SalaryMultiplier>>, Error> { // i32 = job_id
        let r = db.r_transaction()?;
        
        let scan: Vec<SalaryMultiplier> = r
            .scan()
            .primary()?
            .all()?
            .collect::<Result<Vec<_>, _>>()?;

        let multipliers: HashMap<i32, Vec<SalaryMultiplier>> = HashMap::new();

        for job in scan {
            let mut vector=  multipliers[&job.id];
            vector.push(job);
        }

        Ok(multipliers)
    }
    fn apply_modifiers(amount: f32, multipliers: Vec<SalaryMultiplier>) -> MultiplierResult {
    
        let highest_modifiers: Vec<&SalaryMultiplier> = multipliers
            .iter()  // Use iter() instead of into_iter()
            .filter(|modifier| modifier.behavior == MultiplierBehavior::HighestOnly)
            .collect();

        if !highest_modifiers.is_empty() {

            // Look for a multiplier that has a "always_apply" priority
            // First usage - use .iter() to borrow

            let always_apply_multipliers: Vec<&SalaryMultiplier> = highest_modifiers
                .iter()
                .filter(|multiplier| multiplier.priority == MultiplierPriority::AlwaysApply)
                .copied()  // converts &&SalaryMultiplier to &SalaryMultiplier
                .collect();

            let highest_modifier = highest_modifiers
                .into_iter()
                .max_by(|a, b| {
                    a.multiplier
                        .partial_cmp(&b.multiplier)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap()
                .clone();  // Clone the modifier since we only have a reference
            
            let fixed_amount = always_apply_multipliers
                .iter()
                .filter(|f| f.id != highest_modifier.id)
                .fold(amount, |current_amount, m| current_amount * m.multiplier);

            let final_amount = fixed_amount * highest_modifier.multiplier;
            return MultiplierResult {
                taken_amount: amount,
                final_amount: final_amount,
                multipliers: always_apply_multipliers,
                top_multiplier: Some(highest_modifier),
            }
        }
        let final_amount = multipliers
        .iter()
        .fold(amount, |current_amount, m | current_amount * m.multiplier );

        MultiplierResult { 
            taken_amount: amount,
            final_amount: final_amount,
            multipliers: multipliers,
            top_multiplier: None }
    }

    fn new(
        id_gen: &IdGenerator,
        job_id: i32,
        name: String, 
        description: Option<String>, 
        schedule: ReocurrementSchedule, 
        multiplier: f32,
        behavior: MultiplierBehavior,
        priority: MultiplierPriority,
        time_window: Option<TimeWindow>
    ) -> SalaryMultiplier {

            let id = id_gen.next_salary_multiplier_id();

            let multiplier = SalaryMultiplier {
                id: id,
                job_id: job_id,
                behavior: behavior,
                name: name,
                description: description,
                schedule: schedule,
                multiplier: multiplier,
                time_window: time_window,
                priority: priority,
            };

            multiplier
    }
}
impl Job {
    fn load_all(db: &Database) -> Result<HashMap<i32, Job>, Error> {
        let r = db.r_transaction()?;
        
        let jobs: Vec<Job> = r
            .scan()
            .primary()?
            .all()?
            .collect::<Result<Vec<_>, _>>()?;

        let job_map: HashMap<i32, Job> = 
            jobs
            .into_iter()
            .map( |job| {
                (job.id, job)
            })
            .collect();

            Ok(job_map)
    }

    fn new(id_gen: &IdGenerator, name: String, basic_pay: i32, db: &Database) -> Job {

        let id = id_gen.next_job_id();
        let job = Job {
            id: id,
            name,
            basic_pay,
            base_pay_period_hours: None,
            shift_pattern: None,

            multiplier: Self::load_multipliers(id, db),

            overtime_multiplier: None,
            saturday_multiplier: None,
            sunday_multiplier: None,
            bank_holiday_multiplier: None,
            christmass_day_multiplier: None,
            unsociable_hours_multiplier: None,
            unsociable_hours_time_window: None,

            first_day: None,
            fixed_start_time: None,
            fixed_shift_duration: None,
            tax_week_start: None,
        };

        // TODO: Add to the database!

        job
    }
    // Example: basic_pay = 2500 (stored as pence, i.e., £25.00/hour)
    // Rate per second = 2500 / 3600 = 0.694 pence/second   
    fn get_basic_hours_base_rate_per_second(&self) -> f32 {
        self.basic_pay as f32 / 3600.0
    }
    // Returns back the base rate multiplied by the unsociable rate multiplier
    fn get_unsociable_hours_base_rate_per_second(&self) -> f32 {
        self.get_basic_hours_base_rate_per_second() * self.unsociable_hours_multiplier.unwrap_or(1.0)
    }
    // Builder pattern methods for setting optional fields
    fn with_base_hours(mut self, hours: u32) -> Self {
        self.base_pay_period_hours = Some(hours);
        self
    }

    fn with_shift_pattern(mut self, pattern: ShiftPattern) -> Self {
        self.shift_pattern = Some(pattern);
        self
    }

    fn with_overtime_multiplier(mut self, multiplier: f32) -> Self {
        self.overtime_multiplier = Some(multiplier);
        self
    }

    fn with_saturday_multiplier(mut self, multiplier: f32) -> Self {
        self.saturday_multiplier = Some(multiplier);
        self
    }

    fn with_sunday_multiplier(mut self, multiplier: f32) -> Self {
        self.sunday_multiplier = Some(multiplier);
        self
    }

    fn with_bank_holiday_multiplier(mut self, multiplier: f32) -> Self {
        self.bank_holiday_multiplier = Some(multiplier);
        self
    }

    fn with_christmas_day_multiplier(mut self, multiplier: f32) -> Self {
        self.christmass_day_multiplier = Some(multiplier);
        self
    }

    fn with_unsociable_hours_multiplier(mut self, multiplier: f32) -> Self {
        self.unsociable_hours_multiplier = Some(multiplier);
        self
    }

    fn with_first_day(mut self, date: NaiveDate) -> Self {
        self.first_day = Some(date);
        self
    }

    fn with_fixed_start_time(mut self, time: NaiveTime) -> Self {
        self.fixed_start_time = Some(time);
        self
    }

    fn with_fixed_shift_duration(mut self, duration: Duration) -> Self {
        self.fixed_shift_duration = Some(duration);
        self
    }

    fn with_tax_week_start(mut self, start: TaxWeekStart) -> Self {
        self.tax_week_start = Some(start);
        self
    }

    //

    fn get_tax_week_start(&self) -> TaxWeekStart {
        self.tax_week_start.unwrap_or(TaxWeekStart::Sunday)
    }
    fn get_shifts_for_period_of(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        db: &Database
    ) -> Result<Vec<Shift>, db_type::Error> {
        // Get shifts for this specific job within the date range
        Shift::get_shifts_for_period(db, start_date, end_date, Some(self.id))
    }
    
    fn get_all_shifts(&self, db: &Database) -> Result<Vec<Shift>, db_type::Error> {
        // Get entire work history for this job
        Shift::get_all_shifts_for_job(db, self.id)
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
#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
struct ScheduledShift {
    job_id: i32,
    date: NaiveDate,
    status: ShiftStatus,
    day_in_cycle: i32
}
#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
enum ShiftStatus {
    OFF,
    ON
}


#[derive(PartialEq, Serialize, Deserialize, Debug, Clone)]
#[native_model(id = 6, version = 1)]
#[native_db]
struct Shift {
    #[primary_key]
    id: i32,
    
    #[secondary_key]
    job_id: i32,

    date: NaiveDate,
    // This is the key field for date range queries
    #[secondary_key]
    date_key: i32,
    
    shift_type: ShiftType,
    start: NaiveDateTime,
    finish: NaiveDateTime,
}

// saved in the database
#[derive(PartialEq, Serialize, Deserialize, Debug, Clone)]
#[native_model(id = 5, version = 1)]
#[native_db]
struct Deduction {
    #[primary_key]
    id: i32,
    
    #[secondary_key]
    job_id: i32,
    shift_id: i32,

    name: String,
    description: Option<String>,
    amount: u32, // in pence
    
    // Tax treatment
    is_pre_tax: bool, // true = reduces taxable income, false = post-tax deduction
    
    // Scheduling
    schedule: ReocurrementSchedule,
}

#[derive(PartialEq, Serialize, Deserialize, Debug, Clone)]
enum ReocurrementSchedule {
    // One time action on a given date.
    OneTime {
        date: NaiveDate,
        date_key: i32,
        shift_id: Option<i32>, // if tied to a specific shift
    },
    
    // Repeats on specific weekdays (e.g., every Monday and Friday)
    Weekly {
        weekdays: Vec<Weekday>,
        start_date: NaiveDate,
        end_date: Option<NaiveDate>, // None = repeats forever
    },
    
    // Repeats on specific dates each month (e.g., 1st and 15th)
    Monthly {
        day_of_month: Vec<u8>, // 1-31
        start_date: NaiveDate,
        end_date: Option<NaiveDate>,
    },
    
    // Repeats on specific calendar dates (e.g., birthdays, holidays)
    SpecificDates {
        dates: Vec<NaiveDate>,
    },
}


// Fetch specific deductions from the native database --done
// TODO later try to fetch them from global context of dioxus first! --later
impl ReocurrementSchedule {
    fn applies_on(&self, date: NaiveDate) -> bool {
        match self {
            ReocurrementSchedule::OneTime { date: d, .. } => *d == date,
            
            ReocurrementSchedule::Weekly { weekdays, start_date, end_date } => {
                if date < *start_date {
                    return false;
                }
                if let Some(end) = end_date {
                    if date > *end {
                        return false;
                    }
                }
                weekdays.contains(&date.weekday())
            },
            
            ReocurrementSchedule::Monthly { day_of_month, start_date, end_date } => {
                if date < *start_date {
                    return false;
                }
                if let Some(end) = end_date {
                    if date > *end {
                        return false;
                    }
                }
                day_of_month.contains(&(date.day() as u8))
            },
            
            ReocurrementSchedule::SpecificDates { dates } => {
                dates.contains(&date)
            },
        }
    }
}
impl Deduction {
    fn new(
        id_gen: &IdGenerator,
        job_id: i32,
        shift_id: i32,
        name: String,
        description: Option<String>,
        amount: u32,
        is_pre_tax: bool,
        schedule: ReocurrementSchedule) -> Deduction {
            Deduction {
                id: id_gen.next_deduction_id(),
                job_id,
                shift_id,
                name,
                description,
                amount,
                is_pre_tax,
                schedule,
            }
    }
    
    // Get all deductions for a date range
    fn get_deductions_for_period(
        db: &Database,
        job_id: i32,
        start: NaiveDate,
        end: NaiveDate
    ) -> Result<Vec<Deduction>, Error> {
        let r = db.r_transaction()?;
        
        let all_deductions: Vec<Deduction> = r
            .scan()
            .secondary(DeductionKey::job_id)?
            .start_with(job_id)?
            .collect::<Result<Vec<_>, _>>()?;
        
        // Filter to only those that apply in this period
        let applicable: Vec<Deduction> = all_deductions
            .into_iter()
            .filter(|d| {
                let mut current = start;
                while current <= end {
                    if d.schedule.applies_on(current) {
                        return true;
                    }
                    current = current.succ_opt().unwrap();
                }
                false
            })
            .collect();
        
        Ok(applicable)
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]

struct ShiftRecord {
    shift: Shift,
    deductions: Option<Vec<Deduction>>,
    payment: Option<ShiftPayment>,
}

impl Shift {
    fn date_to_key(date: NaiveDate) -> i32 {
        date.year() * 10000 + date.month() as i32 *100 + date.day() as i32
    }
    fn new(
        id: i32,
        job_id: i32,
        date: NaiveDate,
        shift_type: ShiftType,
        start: NaiveDateTime,
        finish: NaiveDateTime,
    ) -> Self {
        Shift {
            id,
            job_id,
            date,
            date_key: Self::date_to_key(date),
            shift_type,
            start,
            finish,
        }
    }

    // Gets all shifts for a given period and then filters by job_id.
    fn get_shifts_for_period(
        db: &Database,
        start_date: NaiveDate,
        end_date: NaiveDate,
        job_id: Option<i32>, // None = all jobs, Some(id) = specific job
    ) -> Result<Vec<Shift>, db_type::Error> {
        let r = db.r_transaction()?;
        
        let start_key = Self::date_to_key(start_date);
        let end_key = Self::date_to_key(end_date);
        
        let shifts: Result<Vec<Shift>, db_type::Error> = r
            .scan()
            .secondary(ShiftKey::date_key)?
            .range(start_key..=end_key)?
            .collect();
        
        let filtered = match job_id {
            Some(id) => shifts?
                .into_iter()
                .filter(|shift| shift.job_id == id)
                .collect(),
            None => shifts?,
        };
        
        Ok(filtered)
    }
    
    // Needed for entire history
    fn get_all_shifts_for_job(
        db: &Database,
        job_id: i32,
    ) -> Result<Vec<Shift>, db_type::Error> {
        let r = db.r_transaction()?;
        
        let shifts: Result<Vec<Shift>, db_type::Error> = r
            .scan()
            .secondary(ShiftKey::job_id)?
            .start_with(job_id)?
            .collect();
        
        shifts
    }
    // Get all shifts for a specific date
    fn get_shifts_for_date(
        db: &Database,
        date: NaiveDate
    ) -> Result<Vec<Shift>, db_type::Error> {
        let r = db.r_transaction()?;
        let date_key = Self::date_to_key(date);
        
        let shifts: Result<Vec<Shift>, db_type::Error> = r
            .scan()
            .secondary(ShiftKey::date_key)?
            .start_with(date_key)?
            .collect();
        
        shifts
    }
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
#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
enum ShiftType {
    Scheduled,
    Sick,
    Holiday,
    PaidLeave,
    ExtraShift
}

// Shift Pay is generated automatically, no need to save in the database!
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]

struct ShiftPayment {
    shift_id: i32,
    job_id: i32,
    amount: u32,
    payment_type: ShiftPaymentType,
    deductions: Option<Vec<Deduction>>, // Only applicable, if set up by the user
}

impl ShiftPayment {
    // Calculate payment for a single shift (WITHOUT overtime)
    fn new_for_shift(shift: &Shift, job: &Job, db: &Database) -> Vec<ShiftPayment> {
        let mut payments = Vec::new();
        
        // Calculate precise seconds worked
        let basic_rate_per_second = job.get_basic_hours_base_rate_per_second();
        let unsociable_rate_per_second: f32 = job.get_unsociable_hours_base_rate_per_second();      

        match shift.shift_type {
            ShiftType::Scheduled | ShiftType::ExtraShift => {
                 let total_seconds_worked = (shift.finish - shift.start).num_seconds();
                // Calculate unsociable seconds FIRST
                let unsociable_seconds = ShiftPayment::calculate_unsociable_seconds(shift, job);
                // Remaining seconds are at basic rate
                let basic_seconds = total_seconds_worked - unsociable_seconds;    
                
                // === DAY-OF-WEEK MULTIPLIERS ===
                // Calculate total base for multiplier (all seconds at basic rate)
                let total_base_amount = basic_seconds as f32 * basic_rate_per_second ;
                let total_unsociable_amount = unsociable_seconds as f32 * unsociable_rate_per_second;
                let weekday = shift.date.weekday();
                
                if weekday == Weekday::Sat && job.saturday_multiplier.is_some() {
                    let multiplier = job.saturday_multiplier.unwrap();

                    let basic_bonus_amount = (total_base_amount * (multiplier)) as u32;
                    let unsociable_bonus_amount = (total_unsociable_amount * (multiplier)) as u32;

                    if unsociable_seconds > 0 && job.unsociable_hours_multiplier.is_some() {
                        payments.push(ShiftPayment {
                            shift_id: shift.id,
                            job_id: shift.job_id,
                            amount: unsociable_bonus_amount as u32,
                            payment_type: ShiftPaymentType::UnsociableSaturday,
                            deductions: None,
                        });
                    }
                    if total_base_amount > 0.0 {
                        payments.push(ShiftPayment {
                            shift_id: shift.id,
                            job_id: shift.job_id,
                            amount: basic_bonus_amount as u32,
                            payment_type: ShiftPaymentType::Saturday,
                            deductions: None,
                        });
                    }
                } else if weekday == Weekday::Sun && job.sunday_multiplier.is_some() {
                    let multiplier = job.sunday_multiplier.unwrap();
                    let basic_bonus_amount = (total_base_amount * (multiplier)) as u32;
                    let unsociable_bonus_amount = (total_unsociable_amount * (multiplier)) as u32;

                    if unsociable_seconds > 0 && job.unsociable_hours_multiplier.is_some() {
                        payments.push(ShiftPayment {
                            shift_id: shift.id,
                            job_id: shift.job_id,
                            amount: unsociable_bonus_amount as u32,
                            payment_type: ShiftPaymentType::UnsociableSunday,
                            deductions: None,
                        });
                    }
                    if total_base_amount > 0.0 {
                        payments.push(ShiftPayment {
                            shift_id: shift.id,
                            job_id: shift.job_id,
                            amount: basic_bonus_amount as u32,
                            payment_type: ShiftPaymentType::Sunday,
                            deductions: None,
                        });
                    }
                } else if BANK_HOLIDAYS.is_bank_holiday(shift.date) && job.bank_holiday_multiplier.is_some() {
                    let multiplier = job.bank_holiday_multiplier.unwrap();
                    let basic_bonus_amount = (total_base_amount * (multiplier)) as u32;
                    let unsociable_bonus_amount = (total_unsociable_amount * (multiplier)) as u32;

                    if unsociable_seconds > 0 && job.unsociable_hours_multiplier.is_some() {
                        payments.push(ShiftPayment {
                            shift_id: shift.id,
                            job_id: shift.job_id,
                            amount: unsociable_bonus_amount as u32,
                            payment_type: ShiftPaymentType::UnsociableBankHoliday,
                            deductions: None,
                        });
                    }
                    if total_base_amount > 0.0 {
                        payments.push(ShiftPayment {
                            shift_id: shift.id,
                            job_id: shift.job_id,
                            amount: basic_bonus_amount as u32,
                            payment_type: ShiftPaymentType::BankHoliday,
                            deductions: None,
                        });
                    }
                } else {
                    if unsociable_seconds > 0 && job.unsociable_hours_multiplier.is_some() {
                        // Pay at the FULL multiplied rate for unsociable time
                        payments.push(ShiftPayment {
                            shift_id: shift.id,
                            job_id: shift.job_id,
                            amount: total_unsociable_amount as u32,
                            payment_type: ShiftPaymentType::UnsociableBasic,
                            deductions: None,
                        });
                    }
                    if total_base_amount > 0.0 {
                        payments.push(ShiftPayment {
                            shift_id: shift.id,
                            job_id: shift.job_id,
                            amount: total_base_amount as u32,
                            payment_type: ShiftPaymentType::Basic,
                            deductions: None,
                        });
                    }

                }
            },
            ShiftType::Sick => {
                let base_amount = 0;
                payments.push(ShiftPayment {
                    shift_id: shift.id,
                    job_id: shift.job_id,
                    amount: base_amount,
                    payment_type: ShiftPaymentType::Sick,
                    deductions: None,
                });
            },
            ShiftType::Holiday | ShiftType::PaidLeave => {
                let base_amount = 0;
                payments.push(ShiftPayment {
                    shift_id: shift.id,
                    job_id: shift.job_id,
                    amount: base_amount,
                    payment_type: ShiftPaymentType::Basic,
                    deductions: None,
                });
            },
        }
     todo!("Add Custom Payment Type! Load from the database and add for the shift.");   
        payments
    }

    

    fn calculate_unsociable_seconds(shift: &Shift, job: &Job) -> i64 {
    // If no unsociable hours configured, return 0
        let (unsociable_start, unsociable_end) = match job.unsociable_hours_time_window {
            Some(window) => window,
            None => return 0,
        };
        
        // Calculate overlap between shift and unsociable window
        Self::calculate_time_overlap_seconds(
            shift.start,
            shift.finish,
            unsociable_start,
            unsociable_end,
        )
    }

}

struct PaymentSummary {
    job_id: i32,
    period_start: NaiveDate,
    period_end: NaiveDate,
    shift_payments: Vec<ShiftPayment>,
    overtime_payments: Vec<ShiftPayment>,
    total_deductions: Vec<ShiftPayment>,
    total_extra: Vec<ShiftPayment>
}
impl PaymentSummary {
    fn new(from: NaiveDate, to: NaiveDate, job_id: i32) -> PaymentSummary {
        let shift_payments: Vec<ShiftPayment> = todo!();
        let overtime_payments: Vec<ShiftPayment> = todo!();
        let total_deductions: Vec<ShiftPayment> = todo!();

        PaymentSummary { 
            job_id: job_id,
            period_start: from,
            period_end: to,
            shift_payments: shift_payments,
            overtime_payments: overtime_payments,
            total_deductions: total_deductions,
          }
    }
    // fn get_gross(&self) -> u32 {
    //     self.payments.iter().map(|payment| {
    //         payment.amount
    //     }).sum()
    // }
    // fn get_tax_paid(&self, region: UKRegion) -> u32 {
    //     self.payments.iter().map(|payment| {
    //         let summary = TaxSummary::new(payment, region);
    //         summary.get_tax_prediction()
    //     }).sum()
    // }
    // fn get_total_deductions(&self, region: UKRegion) -> u32 { // After Tax, so only tax and nin.
    //     self.payments.iter().map(|payment|{
    //         let summary = TaxSummary::new(payment, region);
    //         let tax = summary.get_tax_prediction();
    //         let nin = summary.get_national_insurance_prediction();

    //         tax + nin
    //     }).sum()
    // }
}
#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
enum UKRegion {
    England,
    Wales,
    NorthernIreland,
    Scotland,
}

struct TaxSummary<'a> {
    shift_payment: &'a ShiftPayment,
    region: UKRegion,
}

impl<'a> TaxSummary<'a> {
    fn new(payment: &'a ShiftPayment, region: UKRegion) -> TaxSummary<'a> {
        TaxSummary {
            shift_payment: payment,
            region
        }
    }

    // Calculate income tax based on annual gross income
    // Amounts are in pence (u32), so £12,570 = 1,257,000 pence
    fn get_tax_prediction(&self) -> u32 {
        let annual_gross = self.shift_payment.amount;

        match self.region {
            UKRegion::Scotland => self.calculate_scottish_income_tax(annual_gross),
            UKRegion::England | UKRegion::Wales | UKRegion::NorthernIreland => {
                self.calculate_ruk_income_tax(annual_gross)
            }
        }
    }

    // Calculate Scottish Income Tax (2026/27 rates)
    fn calculate_scottish_income_tax(&self, annual_gross_pence: u32) -> u32 {
        let personal_allowance: u32 = 1_257_000; // £12,570 in pence

        // Apply personal allowance taper for high earners
        let adjusted_allowance = if annual_gross_pence > 10_000_000 {
            // £100,000+ - lose £1 for every £2 over £100k
            let excess = annual_gross_pence.saturating_sub(10_000_000);
            let reduction = excess / 2;
            personal_allowance.saturating_sub(reduction)
        } else {
            personal_allowance
        };

        if annual_gross_pence <= adjusted_allowance {
            return 0;
        }

        let taxable_income = annual_gross_pence - adjusted_allowance;
        let mut tax = 0u32;

        // Starter rate: £12,571 - £16,537 @ 19%
        let starter_limit = 395_700; // (16,537 - 12,570) * 100 = 396,700 pence
        if taxable_income > 0 {
            let taxable_at_starter = taxable_income.min(starter_limit);
            tax += (taxable_at_starter * 19) / 100;
        }

        // Basic rate: £16,538 - £29,526 @ 20%
        let basic_limit = 1_298_900; // (29,526 - 16,537) * 100 = 1,298,900 pence
        if taxable_income > starter_limit {
            let taxable_at_basic = (taxable_income - starter_limit).min(basic_limit);
            tax += (taxable_at_basic * 20) / 100;
        }

        // Intermediate rate: £29,527 - £43,662 @ 21%
        let intermediate_limit = 1_413_600; // (43,662 - 29,526) * 100
        if taxable_income > starter_limit + basic_limit {
            let taxable_at_intermediate = (taxable_income - starter_limit - basic_limit).min(intermediate_limit);
            tax += (taxable_at_intermediate * 21) / 100;
        }

        // Higher rate: £43,663 - £75,000 @ 42%
        let higher_limit = 3_133_800; // (75,000 - 43,662) * 100
        if taxable_income > starter_limit + basic_limit + intermediate_limit {
            let taxable_at_higher = (taxable_income - starter_limit - basic_limit - intermediate_limit).min(higher_limit);
            tax += (taxable_at_higher * 42) / 100;
        }

        // Advanced rate: £75,001 - £125,140 @ 45%
        let advanced_limit = 5_014_000; // (125,140 - 75,000) * 100
        if taxable_income > starter_limit + basic_limit + intermediate_limit + higher_limit {
            let taxable_at_advanced = (taxable_income - starter_limit - basic_limit - intermediate_limit - higher_limit).min(advanced_limit);
            tax += (taxable_at_advanced * 45) / 100;
        }

        // Top rate: Over £125,140 @ 48%
        if taxable_income > starter_limit + basic_limit + intermediate_limit + higher_limit + advanced_limit {
            let taxable_at_top = taxable_income - starter_limit - basic_limit - intermediate_limit - higher_limit - advanced_limit;
            tax += (taxable_at_top * 48) / 100;
        }

        tax
    }

    // Calculate Rest of UK Income Tax (England, Wales, Northern Ireland - 2026/27 rates)
    fn calculate_ruk_income_tax(&self, annual_gross_pence: u32) -> u32 {
        let personal_allowance: u32 = 12_570_00; // £12,570 in pence

        // Apply personal allowance taper for high earners
        let adjusted_allowance = if annual_gross_pence > 10_000_000 {
            let excess = annual_gross_pence.saturating_sub(10_000_000);
            let reduction = excess / 2;
            personal_allowance.saturating_sub(reduction)
        } else {
            personal_allowance
        };

        if annual_gross_pence <= adjusted_allowance {
            return 0;
        }

        let taxable_income = annual_gross_pence - adjusted_allowance;
        let mut tax = 0u32;

        // Basic rate: £12,571 - £50,270 @ 20%
        let basic_limit = 3_770_000; // (50,270 - 12,570) * 100
        if taxable_income > 0 {
            let taxable_at_basic = taxable_income.min(basic_limit);
            tax += (taxable_at_basic * 20) / 100;
        }

        // Higher rate: £50,271 - £125,140 @ 40%
        let higher_limit = 7_487_000; // (125,140 - 50,270) * 100
        if taxable_income > basic_limit {
            let taxable_at_higher = (taxable_income - basic_limit).min(higher_limit);
            tax += (taxable_at_higher * 40) / 100;
        }

        // Additional rate: Over £125,140 @ 45%
        if taxable_income > basic_limit + higher_limit {
            let taxable_at_additional = taxable_income - basic_limit - higher_limit;
            tax += (taxable_at_additional * 45) / 100;
        }

        tax
    }

    // Calculate National Insurance contributions (same across all UK regions)
    // Class 1 Employee rates 2026/27
    fn get_national_insurance_prediction(&self) -> u32 {
        let annual_gross_pence = self.shift_payment.amount;

        // Annual thresholds in pence
        let primary_threshold = 1_257_000; // £12,570 (£242/week * 52)
        let upper_earnings_limit = 5_027_000; // £50,270 (£967/week * 52)

        if annual_gross_pence <= primary_threshold {
            return 0;
        }

        let mut ni = 0u32;

        // 8% on earnings between £12,570 and £50,270
        if annual_gross_pence > primary_threshold {
            let taxable_at_8_percent = (annual_gross_pence - primary_threshold).min(upper_earnings_limit - primary_threshold);
            ni += (taxable_at_8_percent * 8) / 100;
        }

        // 2% on earnings above £50,270
        if annual_gross_pence > upper_earnings_limit {
            let taxable_at_2_percent = annual_gross_pence - upper_earnings_limit;
            ni += (taxable_at_2_percent * 2) / 100;
        }

        ni
    }

    fn get_gross_after_deductions(&self) -> u32 {
        let gross = self.shift_payment.amount;
        let total_deductions = self.get_total_deductions();
        gross.saturating_sub(total_deductions)
    }

    fn get_total_deductions(&self) -> u32 {
        let tax = self.get_tax_prediction();
        let ni = self.get_national_insurance_prediction();

        // Add any custom deductions if present
        let custom_deductions = if let Some(ref deductions) = self.shift_payment.deductions {
            deductions.iter().map(|d| d.amount).sum()
        } else {
            0
        };

        tax + ni + custom_deductions
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
enum ShiftPaymentType {
    Basic,
    UnsociableBasic,
    Sunday,
    UnsociableSunday,
    Saturday,
    UnsociableSaturday,
    Overtime,
    UnsociableOvertime,
    BankHoliday,
    UnsociableBankHoliday,
    Christmass,
    Sick,

    // For example a bonus
    Custom(CustomShiftPaymentType),
}
#[native_model(id = 2, version = 1)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[native_db]
struct CustomShiftPaymentType {
    #[primary_key]
    id: i32,
    #[secondary_key]
    job_id: i32,

    shift_id: i32,

    name: String,
    is_taxable: Option<bool>,
    day: Option<NaiveDate>,
    // The user can chose multiplier and additional amount
    // of money for a custom shift.
    multiplier: Option<f64>,
    amount: Option<u32>,
    schedule: ReocurrementSchedule,
    is_pre_tax: bool, // true = increases taxable income, false = post-tax addition (e.g. bonus)

}
/*

ShiftPayment is generated automatically >
ShiftPaymentType is generated automatically based on the day >
> Therefore CustomShiftPaymentType is the only one that has to be loaded from the database, as...
> It is being saved by the user.

*/

impl CustomShiftPaymentType {
    fn get_reoccuring_payments_for_period(
        db: &Database,
        job_id: i32,
        start: NaiveDate,
        end: NaiveDate
    ) -> Result<Vec<CustomShiftPaymentType>, Error> {
        let r = db.r_transaction()?;
        
        let all_payments: Vec<CustomShiftPaymentType> = r
            .scan()
            .secondary(CustomShiftPaymentTypeKey::job_id)?
            .start_with(job_id)?
            .collect::<Result<Vec<_>, _>>()?;
        
        // Filter to only those that apply in this period
        let applicable: Vec<CustomShiftPaymentType> = all_payments
            .into_iter()
            .filter(|p: &CustomShiftPaymentType| {
                let mut current = start;
                while current <= end {
                    if p.schedule.applies_on(current) {
                        return true;
                    }
                    current = current.succ_opt().unwrap();
                }
                false
            })
            .collect();
        
        Ok(applicable)
    }
    
}


// ID HANDLER FOR ALL ENTITIES

// ID Generator for all entity types
pub struct IdGenerator {
    shift_counter: AtomicI32,
    job_counter: AtomicI32,
    deduction_counter: AtomicI32,
    custom_payment_counter: AtomicI32,
    salary_multiplier_counter: AtomicI32,
}

trait HasId {
    fn id(&self) -> i32;
}

// Maps each type to its counter in IdGenerator
trait HasCounter {
    fn get_counter(generator: &IdGenerator) -> &AtomicI32;
}

impl HasId for SalaryMultiplier {
    fn id(&self) -> i32 {
        self.id
    }
}
impl HasCounter for SalaryMultiplier {
    fn get_counter(generator: &IdGenerator) -> &AtomicI32 {
        &generator.deduction_counter
    }
}
impl HasId for Deduction {
    fn id(&self) -> i32 {
        self.id
    }
}
impl HasCounter for Deduction {
    fn get_counter(generator: &IdGenerator) -> &AtomicI32 {
        &generator.deduction_counter
    }
}

impl HasId for Shift {
    fn id(&self) -> i32 {
        self.id
    }
}
impl HasCounter for Shift {
    fn get_counter(generator: &IdGenerator) -> &AtomicI32 {
        &generator.shift_counter
    }
}

impl HasId for Job {
    fn id(&self) -> i32 {
        self.id
    }
}
impl HasCounter for Job {
    fn get_counter(generator: &IdGenerator) -> &AtomicI32 {
        &generator.job_counter
    }
}

impl HasId for CustomShiftPaymentType {
    fn id(&self) -> i32 {
        self.id
    }
}
impl HasCounter for CustomShiftPaymentType {
    fn get_counter(generator: &IdGenerator) -> &AtomicI32 {
        &generator.custom_payment_counter
    }
}

impl IdGenerator {
    // Create new generator initialized from database
    pub fn new(db: &Database) -> Result<Self, Error> {
        Ok(IdGenerator {
            shift_counter: AtomicI32::new(Self::get_max_id::<Shift>(db)?),
            job_counter: AtomicI32::new(Self::get_max_id::<Job>(db)?),
            deduction_counter: AtomicI32::new(Self::get_max_id::<Deduction>(db)?),
            custom_payment_counter: AtomicI32::new(Self::get_max_id::<CustomShiftPaymentType>(db)?),
            salary_multiplier_counter: AtomicI32::new(Self::get_max_id::<SalaryMultiplier>(db)?)
        })
    }
    fn get_max_id<T>(db: &Database) -> Result<i32, Error> where T: HasId + native_db::ToInput,{
        let r = db.r_transaction()?;
        let max_id = r
            .scan()
            .primary::<T>()?
            .all()?
            .filter_map(|result| result.ok())
            .map(|item| item.id())
            .max()
            .unwrap_or(0);
        Ok(max_id)
    }

    // Generic next_id function that works for any type with HasCounter
    pub fn next_id<T>(&self) -> i32 where T: HasCounter {
        T::get_counter(self).fetch_add(1, Ordering::SeqCst) + 1 as i32
    }

    // Convenience methods (optional - you can use next_id::<Type>() instead)
    pub fn next_shift_id(&self) -> i32 {
        self.next_id::<Shift>()
    }

    pub fn next_job_id(&self) -> i32 {
        self.next_id::<Job>()
    }

    pub fn next_deduction_id(&self) -> i32 {
        self.next_id::<Deduction>()
    }

    pub fn next_custom_payment_id(&self) -> i32 {
        self.next_id::<CustomShiftPaymentType>()
    }
    pub fn next_salary_multiplier_id(&self) -> i32 {
        self.next_id::<SalaryMultiplier>()
    }

}

struct BankHolidayChecker {
    holidays: Vec<BankHoliday>,
    date_set: HashSet<NaiveDate>,
}

impl BankHolidayChecker {
    fn new(holidays: Vec<BankHoliday>) -> Self {
        let date_set = holidays.iter().map(|h| h.date).collect();
        Self { holidays, date_set }
    }
    fn is_bank_holiday(&self, date: NaiveDate) -> bool {
        self.date_set.contains(&date)
    }
    fn get_holiday_on(&self, date: NaiveDate) -> Option<&BankHoliday> {
        if !self.is_bank_holiday(date) { return None }
        self.holidays.iter().find(|holiday| holiday.date == date)
    }
}
struct BankHoliday {
    date: NaiveDate,
    name: String,
}
impl BankHoliday {
    fn new(date: NaiveDate, name: String) -> BankHoliday {
        BankHoliday { date: date, name: name }
    }
}