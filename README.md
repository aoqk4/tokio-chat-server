# tokio-chat-server

## 진행 상황

### tokio-rs를 활용한 비동기 소켓형 채팅 서버
기본적인 채팅 기능 구현

### 연동 클라이언트 개발
https://github.com/aoqk4/tokio-chat-client 참조.

### tracing을 활용한 로깅
간단한 오류처리 핸들링과 알림 로깅작업

### tiberius를 활용한 sql server 연동
기본적인 CRUD를 클라이언트로부터 요청 받아 시행 (현재는 Select 일부 구현 완료)

## 진행 예정

### 현재 코드 모듈화?
Struct - Trait 단위로 묶기

main에 코드를 최대한 줄여보자.

### tonic..axum 등을 활용한 웹 페이지 연동?
grpc나 restapi 를 활용한 웹 로그 테이블 제작

### 테스팅 작업?
DB 관련한 tokio::test 필요성 커진다..
None 값등 상정할 변수가 너무 많음..
