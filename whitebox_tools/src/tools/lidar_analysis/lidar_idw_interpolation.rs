/* 
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: July 3, 2017
Last Modified: December 23, 2017
License: MIT

NOTES: Add the ability to:
Exclude points based on max scan angle divation
Interpolate all LAS files within a directory (i.e. directory input rather than single file).
*/
extern crate time;
extern crate num_cpus;

use std::env;
use std::fs;
use std::f64;
use std::io::{Error, ErrorKind};
use std::path;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use lidar::*;
use raster::*;
use structures::FixedRadiusSearch2D;
use tools::*;

pub struct LidarIdwInterpolation {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl LidarIdwInterpolation {
    pub fn new() -> LidarIdwInterpolation {
        // public constructor
        let name = "LidarIdwInterpolation".to_string();
        let toolbox = "LiDAR Tools".to_string();
        let description = "Interpolates LAS files using an inverse-distance weighted (IDW) scheme."
            .to_string();

        let mut parameters = vec![];
        parameters.push(ToolParameter{
            name: "Input File".to_owned(), 
            flags: vec!["-i".to_owned(), "--input".to_owned()], 
            description: "Input LiDAR file (including extension).".to_owned(),
            parameter_type: ParameterType::ExistingFile(ParameterFileType::Lidar),
            default_value: None,
            optional: true
        });

        parameters.push(ToolParameter{
            name: "Output File".to_owned(), 
            flags: vec!["-o".to_owned(), "--output".to_owned()], 
            description: "Output raster file (including extension).".to_owned(),
            parameter_type: ParameterType::NewFile(ParameterFileType::Raster),
            default_value: None,
            optional: true
        });

        parameters.push(ToolParameter{
            name: "Interpolation Parameter".to_owned(), 
            flags: vec!["--parameter".to_owned()], 
            description: "Interpolation parameter; options are 'elevation' (default), 'intensity', 'class', 'scan angle', 'user data'.".to_owned(),
            parameter_type: ParameterType::OptionList(vec!["elevation".to_owned(), "intensity".to_owned(), "class".to_owned(), "scan angle".to_owned(), "user data".to_owned()]),
            default_value: Some("elevation".to_owned()),
            optional: true
        });

        parameters.push(ToolParameter{
            name: "Point Returns Included".to_owned(), 
            flags: vec!["--returns".to_owned()], 
            description: "Point return types to include; options are 'all' (default), 'last', 'first'.".to_owned(),
            parameter_type: ParameterType::OptionList(vec!["all".to_owned(), "last".to_owned(), "first".to_owned()]),
            default_value: Some("all".to_owned()),
            optional: true
        });

        parameters.push(ToolParameter{
            name: "Grid Resolution".to_owned(), 
            flags: vec!["--resolution".to_owned()], 
            description: "Output raster's grid resolution.".to_owned(),
            parameter_type: ParameterType::Float,
            default_value: Some("1.0".to_owned()),
            optional: true
        });

        parameters.push(ToolParameter{
            name: "IDW Weight (Exponent) Value".to_owned(), 
            flags: vec!["--weight".to_owned()], 
            description: "IDW weight value.".to_owned(),
            parameter_type: ParameterType::Float,
            default_value: Some("1.0".to_owned()),
            optional: true
        });

        parameters.push(ToolParameter{
            name: "Search Radius".to_owned(), 
            flags: vec!["--radius".to_owned()], 
            description: "Search Radius.".to_owned(),
            parameter_type: ParameterType::Float,
            default_value: Some("2.5".to_owned()),
            optional: true
        });
        
        parameters.push(ToolParameter{
            name: "Exclusion Classes (0-18, based on LAS spec; e.g. 3,4,5,6,7)".to_owned(), 
            flags: vec!["--exclude_cls".to_owned()], 
            description: "Optional exclude classes from interpolation; Valid class values range from 0 to 18, based on LAS specifications. Example, --exclude_cls='3,4,5,6,7,18'.".to_owned(),
            parameter_type: ParameterType::String,
            default_value: None,
            optional: true
        });
        
        parameters.push(ToolParameter{
            name: "Palette Name (Whitebox raster outputs only)".to_owned(), 
            flags: vec!["--palette".to_owned()], 
            description: "Optional palette name (for use with Whitebox raster files).".to_owned(),
            parameter_type: ParameterType::String,
            default_value: None,
            optional: true
        });

        parameters.push(ToolParameter{
            name: "Minimum Elevation Value (optional)".to_owned(), 
            flags: vec!["--minz".to_owned()], 
            description: "Optional minimum elevation for inclusion in interpolation.".to_owned(),
            parameter_type: ParameterType::Float,
            default_value: None,
            optional: true
        });
        
        parameters.push(ToolParameter{
            name: "Maximum Elevation Value (optional)".to_owned(), 
            flags: vec!["--maxz".to_owned()], 
            description: "Optional maximum elevation for inclusion in interpolation.".to_owned(),
            parameter_type: ParameterType::Float,
            default_value: None,
            optional: true
        });

        let sep: String = path::MAIN_SEPARATOR.to_string();
        let p = format!("{}", env::current_dir().unwrap().display());
        let e = format!("{}", env::current_exe().unwrap().display());
        let mut short_exe = e.replace(&p, "")
            .replace(".exe", "")
            .replace(".", "")
            .replace(&sep, "");
        if e.contains(".exe") {
            short_exe += ".exe";
        }
        let usage = format!(">>.*{0} -r={1} -v --wd=\"*path*to*data*\" -i=file.las -o=outfile.dep --resolution=2.0 --radius=5.0\"
.*{0} -r={1} --wd=\"*path*to*data*\" -i=file.las -o=outfile.dep --resolution=5.0 --weight=2.0 --radius=2.0 --exclude_cls='3,4,5,6,7,18' --palette=light_quant.plt", short_exe, name).replace("*", &sep);

        LidarIdwInterpolation { 
            name: name, 
            description: description, 
            toolbox: toolbox,
            parameters: parameters, 
            example_usage: usage 
        }
    }
}

impl WhiteboxTool for LidarIdwInterpolation {
    fn get_source_file(&self) -> String {
        String::from(file!())
    }
    
    fn get_tool_name(&self) -> String {
        self.name.clone()
    }

    fn get_tool_description(&self) -> String {
        self.description.clone()
    }

    fn get_tool_parameters(&self) -> String {
        let mut s = String::from("{\"parameters\": [");
        for i in 0..self.parameters.len() {
            if i < self.parameters.len() - 1 {
                s.push_str(&(self.parameters[i].to_string()));
                s.push_str(",");
            } else {
                s.push_str(&(self.parameters[i].to_string()));
            }
        }
        s.push_str("]}");
        s
    }

    fn get_example_usage(&self) -> String {
        self.example_usage.clone()
    }

    fn get_toolbox(&self) -> String {
        self.toolbox.clone()
    }

    fn run<'a>(&self,
               args: Vec<String>,
               working_directory: &'a str,
               verbose: bool)
               -> Result<(), Error> {
        let mut input_file: String = "".to_string();
        let mut output_file: String = "".to_string();
        let mut interp_parameter = "elevation".to_string();
        let mut return_type = "all".to_string();
        let mut grid_res: f64 = 1.0;
        let mut weight = 1.0;
        let mut search_radius = 2.5;
        let mut include_class_vals = vec![true; 256];
        let mut palette = "default".to_string();
        let mut exclude_cls_str = String::new();
        let mut max_z = f64::INFINITY;
        let mut min_z = f64::NEG_INFINITY;

        // read the arguments
        if args.len() == 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "Tool run with no paramters."));
        }
        for i in 0..args.len() {
            let mut arg = args[i].replace("\"", "");
            arg = arg.replace("\'", "");
            let cmd = arg.split("="); // in case an equals sign was used
            let vec = cmd.collect::<Vec<&str>>();
            let mut keyval = false;
            if vec.len() > 1 {
                keyval = true;
            }
            let flag_val = vec[0].to_lowercase().replace("--", "-");
            if flag_val == "-i" || flag_val == "-input" {
                input_file = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-o" || flag_val == "-output" {
                output_file = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-parameter" {
                interp_parameter = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-returns" {
                return_type = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-resolution" {
                grid_res = if keyval {
                    vec[1].to_string().parse::<f64>().unwrap()
                } else {
                    args[i + 1].to_string().parse::<f64>().unwrap()
                };
            } else if flag_val == "-weight" {
                weight = if keyval {
                    vec[1].to_string().parse::<f64>().unwrap()
                } else {
                    args[i + 1].to_string().parse::<f64>().unwrap()
                };
            } else if flag_val == "-radius" {
                search_radius = if keyval {
                    vec[1].to_string().parse::<f64>().unwrap()
                } else {
                    args[i + 1].to_string().parse::<f64>().unwrap()
                };
            } else if flag_val == "-palette" {
                palette = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-exclude_cls" {
                exclude_cls_str = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
                let mut cmd = exclude_cls_str.split(",");
                let mut vec = cmd.collect::<Vec<&str>>();
                if vec.len() == 1 {
                    cmd = exclude_cls_str.split(";");
                    vec = cmd.collect::<Vec<&str>>();
                }
                for value in vec {
                    if !value.trim().is_empty() {
                        let c = value.trim().parse::<usize>().unwrap();
                        include_class_vals[c] = false;
                    }
                }
            } else if flag_val == "-minz" {
                min_z = if keyval {
                    vec[1].to_string().parse::<f64>().unwrap()
                } else {
                    args[i + 1].to_string().parse::<f64>().unwrap()
                };
            } else if flag_val == "-maxz" {
                max_z = if keyval {
                    vec[1].to_string().parse::<f64>().unwrap()
                } else {
                    args[i + 1].to_string().parse::<f64>().unwrap()
                };
            }
        }

        let (all_returns, late_returns, early_returns): (bool, bool, bool);
        if return_type.contains("last") {
            all_returns = false;
            late_returns = true;
            early_returns = false;
        } else if return_type.contains("first") {
            all_returns = false;
            late_returns = false;
            early_returns = true;
        } else {
            // all
            all_returns = true;
            late_returns = false;
            early_returns = false;
        }

        if verbose {
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
            println!("* Welcome to {} *", self.get_tool_name());
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
        }

        let start = time::now();

        let mut inputs = vec![];
        let mut outputs = vec![];
        if input_file.is_empty() {
            if working_directory.is_empty() {
                return Err(Error::new(ErrorKind::InvalidInput,
                    "This tool must be run by specifying either an individual input file or a working directory."));
            }
            match fs::read_dir(working_directory) {
                Err(why) => println!("! {:?}", why.kind()),
                Ok(paths) => for path in paths {
                    let s = format!("{:?}", path.unwrap().path());
                    if s.replace("\"", "").to_lowercase().ends_with(".las") {
                        inputs.push(format!("{:?}", s.replace("\"", "")));
                        outputs.push(inputs[inputs.len()-1].replace(".las", ".tif").replace(".LAS", ".tif"))
                    }
                },
            }
            // return Ok(());
        } else {
            inputs.push(input_file.clone());
            if output_file.is_empty() {
                output_file = input_file.clone().replace(".las", ".tif").replace(".LAS", ".tif");
            }
            outputs.push(output_file);
        }

        for k in 0..inputs.len() {
            input_file = inputs[k].replace("\"", "").clone();
            output_file = outputs[k].replace("\"", "").clone();

            if verbose && inputs.len() > 1 {
                println!("Interpolating {} of {} ({:.2}%) {}", k+1, inputs.len(), (k+1) as f64 / inputs.len() as f64 * 100f64, input_file.clone());
            }

            if !input_file.contains(path::MAIN_SEPARATOR) {
                input_file = format!("{}{}", working_directory, input_file);
            }
            if !output_file.contains(path::MAIN_SEPARATOR) {
                output_file = format!("{}{}", working_directory, output_file);
            }

            if verbose && inputs.len() == 1 {
                println!("Reading input LAS file...");
            }
            let input = match LasFile::new(&input_file, "r") {
                Ok(lf) => lf,
                Err(err) => panic!("Error reading file {}: {}", input_file, err),
            };

            let start_run = time::now();

            if verbose && inputs.len() == 1 {
                println!("Performing analysis...");
            }

            let n_points = input.header.number_of_points as usize;
            let num_points: f64 = (input.header.number_of_points - 1) as f64; // used for progress calculation only

            let mut progress: i32;
            let mut old_progress: i32 = -1;
            let mut frs: FixedRadiusSearch2D<usize> = FixedRadiusSearch2D::new(search_radius);
            let mut interp_vals: Vec<f64> = Vec::with_capacity(n_points);
            match &interp_parameter as &str {
                "elevation" | "z" => {
                    for i in 0..n_points {
                        let p: PointData = input[i];
                        if !p.class_bit_field.withheld() {
                            if all_returns || (p.is_late_return() & late_returns) ||
                            (p.is_early_return() & early_returns) {
                                if include_class_vals[p.classification() as usize] {
                                    if p.z >= min_z && p.z <= max_z {
                                        frs.insert(p.x, p.y, i);
                                    }
                                }
                            }
                        }
                        interp_vals.push(p.z);
                        if verbose {
                            progress = (100.0_f64 * i as f64 / num_points) as i32;
                            if progress != old_progress {
                                println!("Binning points: {}%", progress);
                                old_progress = progress;
                            }
                        }
                    }
                }
                "intensity" => {
                    for i in 0..n_points {
                        let p: PointData = input[i];
                        if !p.class_bit_field.withheld() {
                            if all_returns || (p.is_late_return() & late_returns) ||
                            (p.is_early_return() & early_returns) {
                                if include_class_vals[p.classification() as usize] {
                                    if p.z >= min_z && p.z <= max_z {
                                        frs.insert(p.x, p.y, i);
                                    }
                                }
                            }
                        }
                        interp_vals.push(p.intensity as f64);
                        if verbose {
                            progress = (100.0_f64 * i as f64 / num_points) as i32;
                            if progress != old_progress {
                                println!("Binning points: {}%", progress);
                                old_progress = progress;
                            }
                        }
                    }
                }
                "scan angle" => {
                    for i in 0..n_points {
                        let p: PointData = input[i];
                        if !p.class_bit_field.withheld() {
                            if all_returns || (p.is_late_return() & late_returns) ||
                            (p.is_early_return() & early_returns) {
                                if include_class_vals[p.classification() as usize] {
                                    if p.z >= min_z && p.z <= max_z {
                                        frs.insert(p.x, p.y, i);
                                    }
                                }
                            }
                        }
                        interp_vals.push(p.scan_angle as f64);
                        if verbose {
                            progress = (100.0_f64 * i as f64 / num_points) as i32;
                            if progress != old_progress {
                                println!("Binning points: {}%", progress);
                                old_progress = progress;
                            }
                        }
                    }
                }
                "classs" => {
                    for i in 0..n_points {
                        let p: PointData = input[i];
                        if !p.class_bit_field.withheld() {
                            if all_returns || (p.is_late_return() & late_returns) ||
                            (p.is_early_return() & early_returns) {
                                if include_class_vals[p.classification() as usize] {
                                    if p.z >= min_z && p.z <= max_z {
                                        frs.insert(p.x, p.y, i);
                                    }
                                }
                            }
                        }
                        interp_vals.push(p.classification() as f64);
                        if verbose {
                            progress = (100.0_f64 * i as f64 / num_points) as i32;
                            if progress != old_progress {
                                println!("Binning points: {}%", progress);
                                old_progress = progress;
                            }
                        }
                    }
                }
                _ => {
                    // user data
                    for i in 0..n_points {
                        let p: PointData = input[i];
                        if !p.class_bit_field.withheld() {
                            if all_returns || (p.is_late_return() & late_returns) ||
                            (p.is_early_return() & early_returns) {
                                if include_class_vals[p.classification() as usize] {
                                    if p.z >= min_z && p.z <= max_z {
                                        frs.insert(p.x, p.y, i);
                                    }
                                }
                            }
                        }
                        interp_vals.push(p.user_data as f64);
                        if verbose {
                            progress = (100.0_f64 * i as f64 / num_points) as i32;
                            if progress != old_progress {
                                println!("Binning points: {}%", progress);
                                old_progress = progress;
                            }
                        }
                    }
                }
            }

            let west: f64 = input.header.min_x;
            let north: f64 = input.header.max_y;
            let rows: isize = (((north - input.header.min_y) / grid_res).ceil()) as isize;
            let columns: isize = (((input.header.max_x - west) / grid_res).ceil()) as isize;
            let south: f64 = north - rows as f64 * grid_res;
            let east = west + columns as f64 * grid_res;
            let nodata = -32768.0f64;

            let mut configs = RasterConfigs { ..Default::default() };
            configs.rows = rows as usize;
            configs.columns = columns as usize;
            configs.north = north;
            configs.south = south;
            configs.east = east;
            configs.west = west;
            configs.resolution_x = grid_res;
            configs.resolution_y = grid_res;
            configs.nodata = nodata;
            configs.data_type = DataType::F64;
            configs.photometric_interp = PhotometricInterpretation::Continuous;
            configs.palette = palette.clone();

            let mut output = Raster::initialize_using_config(&output_file, &configs);

            let frs = Arc::new(frs); // wrap FRS in an Arc
            let interp_vals = Arc::new(interp_vals); // wrap interp_vals in an Arc
            let num_procs = num_cpus::get() as isize;
            let row_block_size = rows / num_procs;
            let (tx, rx) = mpsc::channel();
            let mut starting_row;
            let mut ending_row = 0;
            let mut id = 0;
            while ending_row < rows {
                let frs = frs.clone();
                let interp_vals = interp_vals.clone();
                starting_row = id * row_block_size;
                ending_row = starting_row + row_block_size;
                if ending_row > rows {
                    ending_row = rows;
                }
                id += 1;
                let tx1 = tx.clone();
                thread::spawn(move || {
                    let mut index_n: usize;
                    let (mut x, mut y): (f64, f64);
                    let mut zn: f64;
                    let mut dist: f64;
                    let mut val: f64;
                    let mut sum_weights: f64;
                    for row in starting_row..ending_row {
                        let mut data = vec![nodata; columns as usize];
                        for col in 0..columns {
                            x = west + col as f64 * grid_res + 0.5;
                            y = north - row as f64 * grid_res - 0.5;
                            let ret = frs.search(x, y);
                            if ret.len() > 0 {
                                sum_weights = 0.0;
                                val = 0.0;
                                for j in 0..ret.len() {
                                    index_n = ret[j].0;
                                    zn = interp_vals[index_n]; //input[index_n].z;
                                    dist = ret[j].1;
                                    if dist > 0.0 {
                                        val += zn / dist.powf(weight);
                                        sum_weights += 1.0 / dist.powf(weight);
                                    } else {
                                        data[col as usize] = zn;
                                        sum_weights = 0.0;
                                        break;
                                    }
                                }
                                if sum_weights > 0.0 {
                                    data[col as usize] = val / sum_weights;
                                }
                            }
                        }
                        tx1.send((row, data)).unwrap();
                    }
                });
            }

            for row in 0..rows {
                let data = rx.recv().unwrap();
                output.set_row_data(data.0, data.1);
                if verbose {
                    progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as i32;
                    if progress != old_progress {
                        println!("Progress: {}%", progress);
                        old_progress = progress;
                    }
                }
            }

            let end_run = time::now();
            let elapsed_time_run = end_run - start_run;  

            output.add_metadata_entry(format!("Created by whitebox_tools\' {} tool",
                                            self.get_tool_name()));
            output.add_metadata_entry(format!("Input file: {}", input_file));
            output.add_metadata_entry(format!("Grid resolution: {}", grid_res));
            output.add_metadata_entry(format!("Search radius: {}", search_radius));
            output.add_metadata_entry(format!("Weight: {}", weight));
            output.add_metadata_entry(format!("Interpolation parameter: {}", interp_parameter));
            output.add_metadata_entry(format!("Returns: {}", return_type));
            output.add_metadata_entry(format!("Excluded classes: {}", exclude_cls_str));
            output.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time_run).replace("PT", ""));

            if verbose {
                println!("Saving data...")
            };
            let _ = match output.write() {
                Ok(_) => {
                    if verbose {
                        println!("Output file written")
                    }
                }
                Err(e) => return Err(e),
            };
        }

        let end = time::now();
        let elapsed_time = end - start;    

        if verbose {
            println!("{}", &format!("Elapsed Time (including I/O): {}", elapsed_time).replace("PT", ""));
        }

        Ok(())
    }
}
