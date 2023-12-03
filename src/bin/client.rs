use anyhow::Result;
use std::{io::stdin, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::Mutex,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 소켓 생성한다.
    let mut sock_stream = TcpStream::connect("127.0.0.1:8090").await?;

    // ArcMutex로 공유 상태 입력창 하나 띄우고
    let new_line: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

    // 버퍼 입력용 clone
    let input_line = Arc::clone(&new_line);

    // 버퍼 출력(write)용 clone
    let print_line = Arc::clone(&new_line);

    // 서버 끄는 스위치
    let mut server_switch = true;

    // 쓰레드 생성
    let tokio_thread_handle = tokio::spawn(async move {
        // r w로 소켓 분리 하고
        let (reader, mut writer) = sock_stream.split();

        // 버퍼 읽어 들일 준비
        let mut buf_reader = BufReader::new(reader);
        let mut buf: String = String::new();

        // select 루프 시작
        loop {
            tokio::select! {
                // 들어온 버프 읽는다.
                res = buf_reader.read_line(&mut buf) => {
                    if res.unwrap() == 0 {
                        break;
                    }
                    println!("{}", buf.trim());
                    buf.clear();
                }
                //  버퍼 보내기 위한 lock 건다.
                mut cmd_text = print_line.lock() => {
                    writer.write_all(cmd_text.as_bytes()).await.unwrap();

                    // 보낼 것이 있으면 보낸다.
                    if cmd_text.len() > 1 {
                        writer.flush().await.unwrap();

                        // 만약 서버 끄고 싶으면?
                        if cmd_text.trim() == "IWANTEXIT" {

                            // 종료한다고 하고
                            println!("종료됩니다.");

                            // 스위치 끈다.
                            server_switch = false;

                            // break
                            break;
                        }
                        cmd_text.clear();
                    }

                }
            }
        }
    });

    // 버퍼 입력용 & 소켓 유지 위한 루프 시작
    loop {
        // 입력하기 위해서 lock 건다.
        let mut use_input_line = input_line.lock().await;
        // 버퍼 입력
        stdin()
            .read_line(&mut use_input_line)
            .expect("Reading from stdin failed!");

        // 만약 쓰레드 종료 하면?
        if tokio_thread_handle.is_finished() {
            break;
        }
        // 만약 서버 끄고 싶으면?
        if !server_switch {
            break;
        }
    }
    Ok(())
}
