use kurs::ThreadPool;
use std::fs;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;
use std::string::String;
use url_escape::decode;

fn main() {
    let listener = TcpListener::bind(fs::read_to_string("./server.ip").unwrap()).unwrap();
    let pool = ThreadPool::new(4);

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        pool.execute(|id| {
            handle_connection(stream, id);
        });
    }

    println!("Shutting down.");
}

fn handle_connection(mut stream: TcpStream, id: usize) {
    let mut buffer = [0u8; 1048575];
    stream.read(&mut buffer).unwrap();

    let get = b"GET / HTTP/1.1\r\n";
    let post = b"POST / HTTP/1.1\r\n";

    // html response preparation
    let (status_line, filename) = if buffer.starts_with(get) {
        ("HTTP/1.1 200 OK", "hello.html")
    } else if buffer.starts_with(post) {
        ("HTTP/1.1 200 OK", "answer.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404.html")
    };
    let mut server_answer = fs::read_to_string(filename).unwrap();

    let mut log_stream = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open("./message.log")
        .unwrap();

    let buffer_string = String::from_utf8_lossy(&buffer).into_owned();
    let buffer_valuable_end = buffer_string.find("\0").unwrap();

    let code_start = match buffer_string.find("code-input=") {
        Some(usize) => usize + "code-input=".len(),
        None => buffer_valuable_end,
    };

    let code_end = match buffer_string.find("&") {
        Some(usize) => usize,
        None => buffer_valuable_end,
    };

    log_stream.write(&buffer[..buffer_valuable_end]).unwrap();
    log_stream.write(b"\n\n").unwrap();

    if code_start != buffer_valuable_end && code_end != buffer_valuable_end {
        fs::create_dir(format!("./compiler/{}", id)).unwrap();

        let mut cppfile_stream = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(format!("./compiler/{}/main.cpp", id))
            .unwrap();

        // decode url
        let code_string = String::from_utf8_lossy(&buffer[code_start..code_end]).into_owned();
        let code_string = code_string.replace("+", " ");
        let code_string = decode(code_string.as_str()).into_owned();

        cppfile_stream.write(code_string.as_bytes()).unwrap();

        // g++ compiler
        let compile_program =
            Command::new("c:/Users/shoka/RustProjects/kurs/compiler/mingw64/bin/g++.exe")
                .arg(format!("./compiler/{}/main.cpp", id))
                .arg("-o")
                .arg(format!("./compiler/{}/main.exe", id))
                .arg("-static")
                .arg("-static-libgcc")
                .arg("-static-libstdc++")
                .stdout(Stdio::piped())
                .output()
                .expect("failed to execute process");

        let compiler_output = String::from_utf8_lossy(&compile_program.stderr).into_owned();
        let path_string = format!("./compiler/{}/main.exe", id);
        let executable_path = Path::new(&path_string);

        let program_output = if executable_path.exists() {
            let program_executable = Command::new(executable_path)
                .stdout(Stdio::piped())
                .stdin(Stdio::piped())
                .spawn()
                .expect("Failed to write to stdin");

            let program_input_start = match buffer_string.find("program-input") {
                Some(usize) => usize + "program-input".len() + 1usize,
                None => buffer_valuable_end,
            };

            // work with executable
            let program_inputs = &buffer[program_input_start..buffer_valuable_end];

            if program_input_start != buffer_valuable_end {
                program_executable
                    .stdin
                    .unwrap()
                    .write(program_inputs)
                    .expect("Failed to write to stdin");
            }

            let mut program_output_buffer = Vec::new();
            program_executable
                .stdout
                .unwrap()
                .read_to_end(&mut program_output_buffer)
                .expect("Failed to write to stdout");

            String::from_utf8(program_output_buffer).unwrap()
        } else {
            compiler_output
        };

        fs::remove_dir_all(format!("./compiler/{}", id)).expect("Failed to delete directory");

        server_answer.insert_str(
            server_answer.find("\"code-input\"").unwrap() + "\"code-input\"".len() + 1,
            &code_string,
        );

        server_answer.insert_str(
            server_answer.find("\"code-output\"").unwrap() + "\"code-output\"".len() + 1,
            &program_output,
        );
    }

    // html response construction
    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line,
        server_answer.len(),
        server_answer
    );

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}
