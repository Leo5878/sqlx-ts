use color_eyre::eyre::Result;
use convert_case::{Case, Casing};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{self};

use crate::common::lazy::CONFIG;
use crate::common::logger::*;
use crate::ts_generator::errors::TsGeneratorError;

type Array2DContent = Vec<Vec<TsFieldType>>;

#[derive(Debug, Clone)]
pub enum TsFieldType {
    String,
    Number,
    Boolean,
    Object,
    Date,
    Null,
    Any,
    Array2D(Array2DContent),
    Array(Box<TsFieldType>),
    Never,
}

impl fmt::Display for TsFieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TsFieldType::Boolean => write!(f, "boolean"),
            TsFieldType::Number => write!(f, "number"),
            TsFieldType::String => write!(f, "string"),
            TsFieldType::Object => write!(f, "object"),
            TsFieldType::Date => write!(f, "Date"),
            TsFieldType::Any => write!(f, "any"),
            TsFieldType::Null => write!(f, "null"),
            TsFieldType::Never => write!(f, "never"),
            TsFieldType::Array(ts_field_type) => {
                let ts_field_type = ts_field_type.clone();
                let ts_field_type = *ts_field_type;
                write!(f, "Array<{ts_field_type}>")
            }
            TsFieldType::Array2D(nested_array) => {
                let result = nested_array
                    .iter()
                    .map(|items| {
                        let items = items
                            .iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<String>>()
                            .join(", ");

                        format!("[{items}]")
                    })
                    .collect::<Vec<String>>()
                    .join(", ");

                write!(f, "{result}")
            }
        }
    }
}

impl TsFieldType {
    /// The method is to convert the data_type field that you get from PostgreSQL as strings into TsFieldType
    /// so when we stringify TsFieldType, we can correctly translate the data_type into the corresponding TypeScript
    /// data type
    ///
    /// @examples
    /// get_ts_field_type_from_postgres_field_type("integer") -> TsFieldType::Number
    /// get_ts_field_type_from_postgres_field_type("smallint") -> TsFieldType::Number
    ///
    pub fn get_ts_field_type_from_postgres_field_type(field_type: String) -> Self {
        match field_type.as_str() {
            "smallint" | "integer" | "real" | "double precision" | "numeric" => Self::Number,
            "character" | "character varying" | "bytea" | "uuid" | "text" => Self::String,
            "boolean" => Self::Boolean,
            "json" | "jsonb" => Self::Object,
            "ARRAY" | "array" => {
                info!("Currently we cannot figure out the type information for an array, the feature will be added in the future");
                Self::Any
            }
            "date" => Self::Date,
            _ => Self::Any,
        }
    }

    pub fn get_ts_field_type_from_mysql_field_type(mysql_field_type: String) -> Self {
        match mysql_field_type.as_str() {
            "bigint" | "decimal" | "double" | "float" | "int" | "mediumint" | "smallint" | "year" => Self::Number,
            "binary" | "bit" | "blob" | "char" | "text" | "varbinary" | "varchar" => Self::String,
            "tinyint" => Self::Boolean,
            "date" | "datetime" | "timestamp" => Self::Date,
            _ => Self::Any,
        }
    }

    pub fn get_ts_field_from_annotation(annotated_type: &str) -> Self {
        if annotated_type == "string" {
            return Self::String;
        } else if annotated_type == "number" {
            return Self::Number;
        } else if annotated_type == "boolean" {
            return Self::Boolean;
        } else if annotated_type == "object" {
            return Self::Object;
        } else if annotated_type == "null" {
            return Self::Null;
        }
        Self::Any
    }
}

/// TsQuery holds information required to generate typescript type definition
/// of the target SQL query
///
/// There are tests under `tests` folder that checks TsQuery generates the
/// correct type definitions
#[derive(Debug, Clone)]
pub struct TsQuery {
    pub name: String,
    param_order: i32,
    // We use BTreeMap here as it's a collection that's already sorted
    // TODO: use usize instead
    pub params: BTreeMap<i32, TsFieldType>,
    pub annotated_params: BTreeMap<usize, TsFieldType>,

    // We use BTreeMap here as it's a collection that's already sorted
    pub insert_params: BTreeMap<usize, BTreeMap<usize, TsFieldType>>,

    // Holds any annoated @param and perform replacement when generated TS types
    pub annotated_insert_params: BTreeMap<usize, BTreeMap<usize, TsFieldType>>,

    pub result: HashMap<String, Vec<TsFieldType>>,
    // Holds any annotated @result and perform replacement when generating TS types
    pub annotated_results: HashMap<String, Vec<TsFieldType>>,
}

impl TsQuery {
    pub fn new(name: String) -> TsQuery {
        TsQuery {
            name,
            param_order: 0,
            params: BTreeMap::new(),
            annotated_params: BTreeMap::new(),
            result: HashMap::new(),
            insert_params: BTreeMap::new(),
            annotated_results: HashMap::new(),
            annotated_insert_params: BTreeMap::new(),
        }
    }

    /// set annotatd results to ts query so when generating ts types, it can use annotated results wherever possible
    pub fn set_annotated_results(&mut self, annotated_results: HashMap<String, Vec<TsFieldType>>) {
        self.annotated_results = annotated_results;
    }

    pub fn set_annotated_params(&mut self, annotated_params: BTreeMap<usize, TsFieldType>) {
        self.annotated_params = annotated_params;
    }

    pub fn format_column_name(&self, column_name: &str) -> String {
        let convert_to_camel_case_column_name = &CONFIG
            .generate_types_config
            .to_owned()
            .map(|x| x.convert_to_camel_case_column_name);

        match convert_to_camel_case_column_name {
            Some(true) => column_name.to_case(Case::Camel),
            Some(false) | None => column_name.to_string(),
        }
    }

    /// inserts a value into the result hashmap
    /// it should only insert a value if you are working with a non-subquery queries
    pub fn insert_result(
        &mut self,
        alias: Option<&str>,
        value: &[TsFieldType],
        is_selection: bool,
        expr_for_logging: &str,
    ) -> Result<(), TsGeneratorError> {
        if is_selection {
            if let Some(alias) = alias {
                let temp_alias = alias;
                let alias = &self.format_column_name(alias);
                let value = &self
                    .annotated_results
                    .get(temp_alias)
                    .cloned()
                    .unwrap_or_else(|| value.to_vec());

                let _ = &self.result.insert(alias.to_owned(), value.to_owned());
            } else {
                return Err(TsGeneratorError::MissingAliasForFunctions(expr_for_logging.to_string()));
            }
        }
        Ok(())
    }

    /// This is used to insert value params required for INSERT statements
    /// For example if you are given
    ///
    /// e.g.
    /// INSERT INTO table (id, name, address) VALUES (?, 'TEST', ?)
    ///
    /// If you process above MySQL query, it should generate
    ///
    /// e.g.
    /// [ [number, string] ]
    ///
    /// If you are given a query with multiple input values
    ///
    /// e.g.
    /// INSERT INTO table (id, name, address) VALUES (?, 'test', ?), (?, ?, 'address')
    ///
    /// e.g.
    /// [ [number, string], [number, string] ]
    pub fn insert_value_params(&mut self, value: &TsFieldType, point: &(usize, usize), _placeholder: &Option<String>) {
        let (row, column) = point;
        let annotated_insert_param = self.annotated_insert_params.get(row);

        if let Some(annotated_insert_param) = annotated_insert_param {
            let _ = self.insert_params.insert(*row, annotated_insert_param.clone());
        } else {
            let mut row_params = self.insert_params.get_mut(row);

            // If the row of the insert params is not found, create a new BTreeMap and insert it
            if row_params.is_none() {
                let _ = &self.insert_params.insert(*row, BTreeMap::new());
                row_params = self.insert_params.get_mut(row);
            }

            row_params.unwrap().insert(*column, value.to_owned());
        }
    }

    /// Inserts a parameter into TsQuery for type definition generation
    /// If you pass in the order argument, it will use the manually passed in order
    /// It's important to make sure that you are not mixing up the usage
    /// You can only sequentially use `insert_param` with manual order or automatic order parameter
    ///
    /// This method was specifically designed with an assumption that 1 TsQuery is connected to 1 type of DB
    pub fn insert_param(&mut self, value: &TsFieldType, placeholder: &Option<String>) -> Result<(), TsGeneratorError> {
        if let Some(placeholder) = placeholder {
            if placeholder == "?" {
                let annotated_param = self.annotated_params.get(&(self.param_order as usize));

                if let Some(annotated_param) = annotated_param {
                    self.params.insert(self.param_order, annotated_param.clone());
                } else {
                    self.params.insert(self.param_order, value.clone());
                }
                self.param_order += 1;
            } else {
                let re = Regex::new(r"\$(\d+)").unwrap();
                let indexed_binding_params = re.captures(placeholder);

                // Only runs the code if the placeholder is an indexed binding parameter such as $1 or $2
                if let Some(indexed_binding_params) = indexed_binding_params {
                    let order = indexed_binding_params
                        .get(1)
                        .unwrap()
                        .as_str()
                        .parse::<i32>()
                        .unwrap();

                    let annotated_param = self.annotated_params.get(&(order as usize));

                    if let Some(annotated_param) = annotated_param {
                        self.params.insert(order, annotated_param.clone());
                    } else {
                        self.params.insert(order, value.clone());
                    }
                }
            }
        }
        Ok(())
    }

    /// The method is to format SQL params extracted via translate methods
    /// It can work for SELECT, INSERT, DELETE and UPDATE queries
    fn fmt_params(&self, _: &mut fmt::Formatter<'_>) -> String {
        let is_insert_query = self.insert_params.keys().len() > 0;

        if is_insert_query {
            return self
                .insert_params
                .values()
                .map(|row| {
                    // Process each row and produce Number, String, Boolean
                    row.iter()
                        .map(|(_j, col)| col.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                })
                // Wrap the result of row .to_string in `[]`
                .map(|row| format!("[{}]", row))
                .collect::<Vec<String>>()
                .join(", ");
        }

        // Otherwise we should be processing non-insert query params
        let result = &self
            .params
            .to_owned()
            .into_values()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(", ");

        result.to_owned()
    }

    fn fmt_result(&self, _f: &mut fmt::Formatter<'_>) -> String {
        let mut keys = Vec::from_iter(self.result.keys());
        keys.sort();

        let result: Vec<String> = keys
            .iter()
            .map(|key| {
                let data_type = self.result.get(key.to_owned()).unwrap();
                let data_types = data_type
                    .iter()
                    .map(|ts_field_type| ts_field_type.to_string())
                    .collect::<Vec<String>>()
                    .join(" | ");
                format!("{key}: {data_types};")
            })
            .collect();

        result.join("\n\t")
    }
}

impl fmt::Display for TsQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = &self.name;
        let params_str = self.fmt_params(f);
        let result_str = self.fmt_result(f);

        let params = format!(
            r"
export type {name}Params = [{params_str}];
"
        );

        let result = format!(
            r"
export interface I{name}Result {{
    {result_str}
}};
"
        );

        let query = format!(
            r"
export interface I{name}Query {{
    params: {name}Params;
    result: I{name}Result;
}};
"
        );

        let final_code = format!(
            r"
{params}
{result}
{query}"
        );

        writeln!(f, "{}", final_code)
    }
}
