/* 
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: July 4, 2017
Last Modified: July 4, 2017
License: MIT

NOTES: Add anisotropy option.
*/
extern crate time;
extern crate num_cpus;

use std::env;
use std::path;
use std::f64;
use raster::*;
use std::io::{Error, ErrorKind};
use tools::WhiteboxTool;

pub struct CostPathway {
    name: String,
    description: String,
    parameters: String,
    example_usage: String,
}

impl CostPathway {
    pub fn new() -> CostPathway { // public constructor
        let name = "CostPathway".to_string();
        
        let description = "Performs cost-distance pathway analysis using a series of destination grid cells.".to_string();
        
        let mut parameters = "--destination   Input destination raster file.\n".to_owned();
        parameters.push_str("--backlink      Input backlink raster file.\n");
        parameters.push_str("-o, --output    Output cost pathway raster file.\n");
        
        let sep: String = path::MAIN_SEPARATOR.to_string();
        let p = format!("{}", env::current_dir().unwrap().display());
        let e = format!("{}", env::current_exe().unwrap().display());
        let mut short_exe = e.replace(&p, "").replace(".exe", "").replace(".", "").replace(&sep, "");
        if e.contains(".exe") {
            short_exe += ".exe";
        }
        let usage = format!(">>.*{0} -r={1} --wd=\"*path*to*data*\" --destination=dst.dep --backlink=backlink.dep --output=cost_path.dep", short_exe, name).replace("*", &sep);
    
        CostPathway { name: name, description: description, parameters: parameters, example_usage: usage }
    }
}

impl WhiteboxTool for CostPathway {
    fn get_tool_name(&self) -> String {
        self.name.clone()
    }

    fn get_tool_description(&self) -> String {
        self.description.clone()
    }

    fn get_tool_parameters(&self) -> String {
        self.parameters.clone()
    }

    fn get_example_usage(&self) -> String {
        self.example_usage.clone()
    }

    fn run<'a>(&self, args: Vec<String>, working_directory: &'a str, verbose: bool) -> Result<(), Error> {
        let mut destination_file = String::new();
        let mut backlink_file = String::new();
        let mut output_file = String::new();
        
        if args.len() == 0 {
            return Err(Error::new(ErrorKind::InvalidInput,
                                "Tool run with no paramters. Please see help (-h) for parameter descriptions."));
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
            if vec[0].to_lowercase() == "-destination" || vec[0].to_lowercase() == "--destination" {
                if keyval {
                    destination_file = vec[1].to_string();
                } else {
                    destination_file = args[i+1].to_string();
                }
            } else if vec[0].to_lowercase() == "-backlink" || vec[0].to_lowercase() == "--backlink" {
                if keyval {
                    backlink_file = vec[1].to_string();
                } else {
                    backlink_file = args[i+1].to_string();
                }
            } else if vec[0].to_lowercase() == "-o" || vec[0].to_lowercase() == "--output" {
                if keyval {
                    output_file = vec[1].to_string();
                } else {
                    output_file = args[i+1].to_string();
                }
            }
        }

        if verbose {
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
            println!("* Welcome to {} *", self.get_tool_name());
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
        }

        let sep: String = path::MAIN_SEPARATOR.to_string();

        let mut progress: usize;
        let mut old_progress: usize = 1;

        if !destination_file.contains(&sep) {
            destination_file = format!("{}{}", working_directory, destination_file);
        }
        if !backlink_file.contains(&sep) {
            backlink_file = format!("{}{}", working_directory, backlink_file);
        }
        if !output_file.contains(&sep) {
            output_file = format!("{}{}", working_directory, output_file);
        }
        
        if verbose { println!("Reading destination data...") };
        let destination = Raster::new(&destination_file, "r")?;

        if verbose { println!("Reading backlink data...") };
        let backlink = Raster::new(&backlink_file, "r")?;

        // make sure the input files have the same size
        if destination.configs.rows != backlink.configs.rows || destination.configs.columns != backlink.configs.columns {
            return Err(Error::new(ErrorKind::InvalidInput,
                                "The input files must have the same number of rows and columns and spatial extent."));
        }

        let start = time::now();
        let rows = destination.configs.rows as isize;
        let columns = destination.configs.columns as isize;
        let nodata = backlink.configs.nodata;
        
        let mut output = Raster::initialize_using_file(&output_file, &destination);
        
        let dx = [ 1, 1, 1, 0, -1, -1, -1, 0 ];
        let dy = [ -1, 0, 1, 1, 1, 0, -1, -1 ];
        let mut pntr_matches: [usize; 129] = [0usize; 129];
        // This maps Whitebox-style D8 pointer values
        // onto the cell offsets in dx and dy.
        pntr_matches[1] = 0usize;
        pntr_matches[2] = 1usize;
        pntr_matches[4] = 2usize;
        pntr_matches[8] = 3usize;
        pntr_matches[16] = 4usize;
        pntr_matches[32] = 5usize;
        pntr_matches[64] = 6usize;
        pntr_matches[128] = 7usize;
        let (mut x, mut y): (isize, isize);
        let mut flag: bool;
        let mut dir: f64;
        for row in 0..rows {
            for col in 0..columns {
                if destination[(row, col)] > 0.0 && backlink[(row, col)] != nodata {
                    flag = false;
                    x = col;
                    y = row;
                    while !flag {
                        if output[(y, x)] == nodata {
                            output[(y, x)] = 1.0;
                        } else {
                            output.increment(y, x, 1.0);
                        }
                        // find it's downslope neighbour
                        dir = backlink[(y, x)];
                        if dir != nodata && dir > 0.0 {
                            // move x and y accordingly
                            x += dx[pntr_matches[dir as usize]];
                            y += dy[pntr_matches[dir as usize]];
                        } else {
                            flag = true;
                        }
                    }

                }
            }
            if verbose {
                progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
                if progress != old_progress {
                    println!("Progress: {}%", progress);
                    old_progress = progress;
                }
            }
        }

        
        let end = time::now();
        let elapsed_time = end - start;
        output.configs.palette = "spectrum.plt".to_string();
        output.configs.data_type = DataType::F32;
        output.configs.photometric_interp = PhotometricInterpretation::Continuous;
        output.add_metadata_entry(format!("Created by whitebox_tools\' {} tool", self.get_tool_name()));
        output.add_metadata_entry(format!("Destination raster file: {}", destination_file));
        output.add_metadata_entry(format!("Backlink raster: {}", backlink_file));
        output.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time).replace("PT", ""));

        if verbose { println!("Saving data...") };
        let _ = match output.write() {
            Ok(_) => if verbose { println!("Output file written") },
            Err(e) => return Err(e),
        };

        println!("{}", &format!("Elapsed Time (excluding I/O): {}", elapsed_time).replace("PT", ""));

        Ok(())
    }
}
