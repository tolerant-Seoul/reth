# WBFT Consensus 포팅 구현 체크리스트

## 개요

이 문서는 go-stablenet의 WBFT 합의 알고리즘을 reth로 포팅하기 위한 우선순위가 정해진 실행 가능한 작업 목록입니다.

**프로젝트 기간**: 약 3-4개월 (1명 full-time 기준)
**구현 위치**: `/Users/wm-it-22-00661/Work/github/stable-net/porting-reth/reth`

---

## Phase 1: 기초 구조 및 데이터 타입 (2-3주)

### 1.1 프로젝트 구조 설정 ✅ 우선순위: P0

**목표**: 기본 크레이트 구조 및 의존성 설정

- [ ] **작업 1.1.1**: WBFT 크레이트 생성
  - 위치: `crates/consensus/wbft/`
  - 생성 파일:
    - `Cargo.toml` - 의존성 정의
    - `src/lib.rs` - 크레이트 진입점
    - `README.md` - 크레이트 설명
  - 의존성 추가:
    ```toml
    [dependencies]
    blst = "0.3"
    tokio = { version = "1", features = ["full"] }
    alloy-rlp = "0.3"
    reth-primitives = { workspace = true }
    reth-consensus = { workspace = true }
    metrics = "0.21"
    tracing = "0.1"
    ```
  - 예상 시간: 1일
  - 검증: `cargo check -p reth-consensus-wbft` 성공

- [ ] **작업 1.1.2**: 워크스페이스에 크레이트 등록
  - 파일: `Cargo.toml` (루트)
  - 추가 내용:
    ```toml
    [workspace]
    members = [
        # ... 기존 멤버들
        "crates/consensus/wbft",
    ]
    ```
  - 예상 시간: 0.5일
  - 검증: `cargo build --workspace` 성공

---

### 1.2 기본 데이터 구조 ✅ 우선순위: P0

**목표**: WBFT 프로토콜의 핵심 데이터 타입 정의

- [ ] **작업 1.2.1**: View 타입 구현
  - 파일: `crates/consensus/wbft/src/types.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
    pub struct View {
        pub round: U256,
        pub sequence: U256,
    }

    impl View {
        pub fn new(round: U256, sequence: U256) -> Self { ... }
        pub fn cmp(&self, other: &View) -> Ordering { ... }
    }
    ```
  - 테스트 작성:
    - `test_view_encoding` - RLP 인코딩/디코딩
    - `test_view_comparison` - 순서 비교
  - 예상 시간: 1일
  - 검증: `cargo test -p reth-consensus-wbft types::view`

- [ ] **작업 1.2.2**: Subject 타입 구현
  - 파일: `crates/consensus/wbft/src/types.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct Subject {
        pub view: View,
        pub digest: B256,
    }
    ```
  - 예상 시간: 0.5일

- [ ] **작업 1.2.3**: State enum 정의
  - 파일: `crates/consensus/wbft/src/state.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum State {
        AcceptRequest,
        Preprepared,
        Prepared,
        Committed,
    }
    ```
  - 예상 시간: 0.5일

---

### 1.3 BLS 서명 기초 구현 ✅ 우선순위: P0

**목표**: BLS12-381 서명 기본 기능

- [ ] **작업 1.3.1**: BLS 모듈 설정 및 키 타입
  - 파일: `crates/consensus/wbft/src/bls/mod.rs`
  - 구현 내용:
    ```rust
    pub struct SecretKey(blst::min_pk::SecretKey);
    pub struct PublicKey(blst::min_pk::PublicKey);
    pub struct Signature(blst::min_pk::Signature);

    impl SecretKey {
        pub fn random() -> Self { ... }
        pub fn from_bytes(bytes: &[u8]) -> Result<Self> { ... }
        pub fn to_bytes(&self) -> [u8; 32] { ... }
        pub fn public_key(&self) -> PublicKey { ... }
    }
    ```
  - 예상 시간: 2일
  - 검증: 키 생성 및 직렬화 테스트

- [ ] **작업 1.3.2**: 개별 서명 생성 및 검증
  - 파일: `crates/consensus/wbft/src/bls/sign.rs`
  - 구현 내용:
    ```rust
    impl SecretKey {
        pub fn sign(&self, message: &[u8]) -> Signature { ... }
    }

    impl PublicKey {
        pub fn verify(&self, message: &[u8], sig: &Signature) -> bool { ... }
    }
    ```
  - 테스트:
    - `test_sign_verify` - 기본 서명/검증
    - `test_invalid_signature` - 잘못된 서명 거부
  - 예상 시간: 2일
  - 검증: 모든 BLS 서명 테스트 통과

- [ ] **작업 1.3.3**: 서명 직렬화
  - 파일: `crates/consensus/wbft/src/bls/serialize.rs`
  - 구현 내용:
    ```rust
    impl Signature {
        pub fn to_bytes(&self) -> [u8; 96] { ... }
        pub fn from_bytes(bytes: &[u8]) -> Result<Self> { ... }
    }

    impl PublicKey {
        pub fn to_bytes(&self) -> [u8; 48] { ... }
        pub fn from_bytes(bytes: &[u8]) -> Result<Self> { ... }
    }
    ```
  - 예상 시간: 1일
  - 검증: 직렬화 왕복 테스트

---

### 1.4 메시지 데이터 타입 ✅ 우선순위: P0

**목표**: 4가지 합의 메시지 타입 정의

- [ ] **작업 1.4.1**: 메시지 공통 trait 정의
  - 파일: `crates/consensus/wbft/src/messages/mod.rs`
  - 구현 내용:
    ```rust
    pub trait WbftMessage: RlpEncodable + RlpDecodable {
        fn code() -> u8;
        fn view(&self) -> &View;
        fn source(&self) -> Address;
        fn set_source(&mut self, addr: Address);
        fn signature(&self) -> &[u8];
        fn set_signature(&mut self, sig: Vec<u8>);
        fn encode_for_signing(&self) -> Vec<u8>;
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 1.4.2**: PRE-PREPARE 메시지
  - 파일: `crates/consensus/wbft/src/messages/preprepare.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct Preprepare {
        pub view: View,
        pub proposal: SealedBlock,
        pub round_change_justification: Vec<RoundChange>,
        pub source: Address,
        pub signature: Vec<u8>,
    }

    impl WbftMessage for Preprepare { ... }
    ```
  - 테스트: RLP 인코딩/디코딩, 서명 메시지 생성
  - 예상 시간: 2일

- [ ] **작업 1.4.3**: PREPARE 메시지
  - 파일: `crates/consensus/wbft/src/messages/prepare.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct Prepare {
        pub view: View,
        pub digest: B256,
        pub bls_seal: Vec<u8>,  // 96 bytes
        pub source: Address,
        pub signature: Vec<u8>,
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 1.4.4**: COMMIT 메시지
  - 파일: `crates/consensus/wbft/src/messages/commit.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct Commit {
        pub view: View,
        pub digest: B256,
        pub bls_seal: Vec<u8>,  // 96 bytes
        pub source: Address,
        pub signature: Vec<u8>,
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 1.4.5**: ROUND-CHANGE 메시지
  - 파일: `crates/consensus/wbft/src/messages/roundchange.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct RoundChange {
        pub view: View,
        pub prepared_round: Option<U256>,
        pub prepared_digest: Option<B256>,
        pub justification: Vec<Prepare>,
        pub source: Address,
        pub signature: Vec<u8>,
    }
    ```
  - 예상 시간: 2일

---

## Phase 2: 합의 프로토콜 핵심 로직 (3-4주)

### 2.1 BLS 서명 집계 ✅ 우선순위: P0

**목표**: 다중 서명 집계 및 검증

- [ ] **작업 2.1.1**: SealerSet (비트맵) 구현
  - 파일: `crates/consensus/wbft/src/bls/sealer_set.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct SealerSet(Vec<u8>);

    impl SealerSet {
        pub fn new(size: usize) -> Self { ... }
        pub fn set_sealer(&mut self, index: u32) { ... }
        pub fn is_sealer(&self, index: u32) -> bool { ... }
        pub fn get_sealers(&self) -> Vec<u32> { ... }
        pub fn count(&self) -> usize { ... }
    }
    ```
  - 테스트: 비트맵 설정/조회, 카운트
  - 예상 시간: 1일

- [ ] **작업 2.1.2**: WBFTAggregatedSeal 타입
  - 파일: `crates/consensus/wbft/src/bls/aggregated_seal.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct WbftAggregatedSeal {
        pub bitmap: SealerSet,
        pub signature: [u8; 96],
    }

    impl WbftAggregatedSeal {
        pub fn verify(&self, public_keys: &[PublicKey], message: &[u8]) -> bool { ... }
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 2.1.3**: 서명 집계 로직
  - 파일: `crates/consensus/wbft/src/bls/aggregate.rs`
  - 구현 내용:
    ```rust
    pub fn aggregate_signatures(signatures: &[Signature]) -> Result<Signature> {
        // blst aggregate 사용
    }

    pub fn verify_aggregated(
        public_keys: &[PublicKey],
        message: &[u8],
        signature: &Signature
    ) -> bool {
        // Fast aggregate verification
    }
    ```
  - 테스트:
    - `test_aggregate_2_signatures`
    - `test_aggregate_4_signatures`
    - `test_aggregate_verification`
  - 예상 시간: 2일
  - 검증: 3, 4, 7 검증자 집계 시나리오 테스트

---

### 2.2 ValidatorSet 구현 ✅ 우선순위: P0

**목표**: 검증자 세트 관리 및 제안자 선출

- [ ] **작업 2.2.1**: Validator trait 정의
  - 파일: `crates/consensus/wbft/src/validator/mod.rs`
  - 구현 내용:
    ```rust
    pub trait Validator: Clone + Send + Sync {
        fn address(&self) -> Address;
        fn bls_public_key(&self) -> &[u8];
    }

    #[derive(Debug, Clone)]
    pub struct DefaultValidator {
        addr: Address,
        bls_key: Vec<u8>,
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 2.2.2**: ValidatorSet trait 및 구현
  - 파일: `crates/consensus/wbft/src/validator/set.rs`
  - 구현 내용:
    ```rust
    pub trait ValidatorSet: Send + Sync {
        fn calc_proposer(&mut self, last_proposer: Address, round: u64);
        fn size(&self) -> usize;
        fn list(&self) -> &[Box<dyn Validator>];
        fn get_proposer(&self) -> &dyn Validator;
        fn f(&self) -> f64 { (self.size() - 1) as f64 / 3.0 }
        fn quorum_size(&self) -> usize {
            (self.size() as f64 - self.f()).ceil() as usize
        }
    }

    pub struct DefaultValidatorSet { ... }
    ```
  - 테스트:
    - `test_quorum_size` - 3,4,7,10 검증자
    - `test_f_calculation`
  - 예상 시간: 2일

- [ ] **작업 2.2.3**: 제안자 선출 정책
  - 파일: `crates/consensus/wbft/src/validator/policy.rs`
  - 구현 내용:
    ```rust
    pub enum ProposerPolicy {
        RoundRobin,
        Sticky,
    }

    pub fn calc_proposer(
        policy: ProposerPolicy,
        val_set: &ValidatorSet,
        last_proposer: Address,
        round: u64
    ) -> &dyn Validator {
        match policy {
            ProposerPolicy::RoundRobin => round_robin_proposer(...),
            ProposerPolicy::Sticky => sticky_proposer(...),
        }
    }
    ```
  - 테스트: 제안자 순환 검증
  - 예상 시간: 2일

---

### 2.3 합의 상태 머신 ✅ 우선순위: P0

**목표**: 핵심 3단계 합의 프로토콜 구현

- [ ] **작업 2.3.1**: Core 상태 구조체
  - 파일: `crates/consensus/wbft/src/core/mod.rs`
  - 구현 내용:
    ```rust
    pub struct Core {
        state: State,
        current_view: View,
        validator_set: Arc<RwLock<dyn ValidatorSet>>,
        backend: Arc<dyn Backend>,
        message_set: MessageSet,
        round_change_set: RoundChangeSet,
        current_preprepare: Option<Preprepare>,
        pending_request: Option<Request>,
        // 채널 및 타이머
    }

    impl Core {
        pub async fn start(&mut self) { ... }
        pub async fn stop(&mut self) { ... }
    }
    ```
  - 예상 시간: 2일

- [ ] **작업 2.3.2**: PRE-PREPARE 단계 핸들러
  - 파일: `crates/consensus/wbft/src/core/preprepare.rs`
  - 구현 내용:
    ```rust
    impl Core {
        pub async fn handle_preprepare(&mut self, msg: &Preprepare) -> Result<()> {
            // 1. 제안자 검증
            // 2. 라운드 변경 정당화 검증 (해당시)
            // 3. 블록 기본 검증
            // 4. PREPARE 메시지 브로드캐스트
            // 5. 상태 전환: AcceptRequest -> Preprepared
        }

        pub async fn send_preprepare(&mut self, request: Request) -> Result<()> {
            // 제안자만 호출
        }
    }
    ```
  - 예상 시간: 3일

- [ ] **작업 2.3.3**: PREPARE 단계 핸들러
  - 파일: `crates/consensus/wbft/src/core/prepare.rs`
  - 구현 내용:
    ```rust
    impl Core {
        pub async fn handle_prepare(&mut self, msg: &Prepare) -> Result<()> {
            // 1. BLS 서명 검증
            // 2. MessageSet에 추가
            // 3. Quorum 확인
            // 4. COMMIT 메시지 브로드캐스트
            // 5. 상태 전환: Preprepared -> Prepared
        }
    }
    ```
  - 예상 시간: 3일

- [ ] **작업 2.3.4**: COMMIT 단계 핸들러
  - 파일: `crates/consensus/wbft/src/core/commit.rs`
  - 구현 내용:
    ```rust
    impl Core {
        pub async fn handle_commit(&mut self, msg: &Commit) -> Result<()> {
            // 1. BLS 서명 검증
            // 2. MessageSet에 추가
            // 3. Quorum 확인
            // 4. 서명 집계
            // 5. 블록 완료 (backend.commit)
            // 6. 상태 전환: Prepared -> Committed
        }
    }
    ```
  - 예상 시간: 3일

- [ ] **작업 2.3.5**: MessageSet 구현
  - 파일: `crates/consensus/wbft/src/core/message_set.rs`
  - 구현 내용:
    ```rust
    pub struct MessageSet {
        view: View,
        messages: HashMap<Address, Vec<u8>>,  // BLS seals
    }

    impl MessageSet {
        pub fn add(&mut self, msg: &dyn WbftMessage) -> Result<()> { ... }
        pub fn size(&self) -> usize { ... }
        pub fn has_quorum(&self, quorum_size: usize) -> bool { ... }
        pub fn get_seals(&self) -> Vec<Vec<u8>> { ... }
    }
    ```
  - 예상 시간: 2일

---

### 2.4 라운드 변경 메커니즘 ✅ 우선순위: P1

**목표**: 타임아웃 및 라운드 변경 처리

- [ ] **작업 2.4.1**: 타이머 관리
  - 파일: `crates/consensus/wbft/src/core/timer.rs`
  - 구현 내용:
    ```rust
    pub struct RoundChangeTimer {
        timeout: Duration,
        handle: Option<JoinHandle<()>>,
    }

    impl RoundChangeTimer {
        pub fn new(base_timeout: Duration, round: u64) -> Self {
            // 지수 백오프: timeout = base * 2^round
        }

        pub async fn start(&mut self) -> oneshot::Receiver<()> { ... }
        pub fn stop(&mut self) { ... }
    }
    ```
  - 예상 시간: 2일

- [ ] **작업 2.4.2**: ROUND-CHANGE 핸들러
  - 파일: `crates/consensus/wbft/src/core/roundchange.rs`
  - 구현 내용:
    ```rust
    impl Core {
        pub async fn handle_timeout(&mut self) -> Result<()> {
            // 1. ROUND-CHANGE 메시지 생성
            // 2. 준비된 블록 포함 (해당시)
            // 3. PREPARE 정당화 포함
            // 4. 브로드캐스트
        }

        pub async fn handle_round_change(&mut self, msg: &RoundChange) -> Result<()> {
            // 1. 정당화 검증
            // 2. RoundChangeSet에 추가
            // 3. F+1 도달시 라운드 이동
            // 4. Quorum 도달시 PRE-PREPARE 전송 (제안자인 경우)
        }
    }
    ```
  - 예상 시간: 4일

- [ ] **작업 2.4.3**: RoundChangeSet 구현
  - 파일: `crates/consensus/wbft/src/core/roundchange_set.rs`
  - 구현 내용:
    ```rust
    pub struct RoundChangeSet {
        round_changes: HashMap<U256, HashMap<Address, RoundChange>>,
    }

    impl RoundChangeSet {
        pub fn add(&mut self, msg: RoundChange) -> Result<()> { ... }
        pub fn has_f_plus_one(&self, round: &U256, f: usize) -> bool { ... }
        pub fn has_quorum(&self, round: &U256, quorum: usize) -> bool { ... }
        pub fn get_prepared_block(&self, round: &U256) -> Option<SealedBlock> { ... }
    }
    ```
  - 예상 시간: 2일

---

### 2.5 백로그 및 미래 메시지 처리 ✅ 우선순위: P2

**목표**: 미래 블록/라운드 메시지 버퍼링

- [ ] **작업 2.5.1**: Backlog 구현
  - 파일: `crates/consensus/wbft/src/core/backlog.rs`
  - 구현 내용:
    ```rust
    pub struct Backlog {
        messages: VecDeque<(u8, Vec<u8>)>,  // (code, payload)
        max_size: usize,
    }

    impl Backlog {
        pub fn store(&mut self, code: u8, msg: Vec<u8>) { ... }
        pub fn process_backlog(&mut self, current_view: &View) -> Vec<(u8, Vec<u8>)> { ... }
    }
    ```
  - 예상 시간: 2일

---

## Phase 3: 검증 및 reth 통합 (3-4주)

### 3.1 블록 헤더 Extra Data ✅ 우선순위: P0

**목표**: WBFT 합의 증명을 헤더에 저장

- [ ] **작업 3.1.1**: WBFTExtra 구조체
  - 파일: `crates/consensus/wbft/src/header.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct WbftExtra {
        pub vanity_data: [u8; 32],
        pub randao_reveal: Vec<u8>,
        pub prev_round: u32,
        pub prev_prepared_seal: Option<WbftAggregatedSeal>,
        pub prev_committed_seal: Option<WbftAggregatedSeal>,
        pub round: u32,
        pub prepared_seal: Option<WbftAggregatedSeal>,
        pub committed_seal: Option<WbftAggregatedSeal>,
        pub gas_tip: U256,
        pub epoch_info: Option<EpochInfo>,
    }

    impl WbftExtra {
        pub fn encode(&self) -> Vec<u8> { ... }
        pub fn decode(data: &[u8]) -> Result<Self> { ... }
    }
    ```
  - 예상 시간: 2일

- [ ] **작업 3.1.2**: PrepareSeal 해싱
  - 파일: `crates/consensus/wbft/src/header.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, Copy)]
    pub enum SealType {
        Prepare = 0,
        Commit = 1,
    }

    pub fn prepare_seal(
        header: &Header,
        round: u32,
        seal_type: SealType
    ) -> B256 {
        // 1. header.hash_with_round_number(round)
        // 2. append seal_type byte
        // 3. keccak256
    }
    ```
  - 예상 시간: 1일

---

### 3.2 Consensus Trait 구현 ✅ 우선순위: P0

**목표**: reth의 Consensus trait 구현

- [ ] **작업 3.2.1**: WbftConsensus 구조체
  - 파일: `crates/consensus/wbft/src/consensus.rs`
  - 구현 내용:
    ```rust
    pub struct WbftConsensus<ChainSpec> {
        chain_spec: Arc<ChainSpec>,
        config: WbftConfig,
    }

    impl<ChainSpec> WbftConsensus<ChainSpec> {
        pub fn new(chain_spec: Arc<ChainSpec>, config: WbftConfig) -> Self { ... }
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 3.2.2**: Consensus trait - validate_body_against_header
  - 파일: `crates/consensus/wbft/src/consensus.rs`
  - 구현 내용:
    ```rust
    impl<B: Block> Consensus<B> for WbftConsensus<ChainSpec> {
        type Error = ConsensusError;

        fn validate_body_against_header(
            &self,
            body: &B::Body,
            header: &SealedHeader<B::Header>
        ) -> Result<(), Self::Error> {
            // 기존 검증 재사용
            validate_body_against_header(body, header.header())
        }
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 3.2.3**: Consensus trait - validate_block_pre_execution
  - 파일: `crates/consensus/wbft/src/consensus.rs`
  - 구현 내용:
    ```rust
    fn validate_block_pre_execution(&self, block: &SealedBlock<B>) -> Result<(), Self::Error> {
        // 기본 검증
        validate_block_pre_execution(block, &self.chain_spec)?;

        // WBFT 특화 검증은 헤더 검증에서 수행
        Ok(())
    }
    ```
  - 예상 시간: 1일

---

### 3.3 HeaderValidator Trait 구현 ✅ 우선순위: P0

**목표**: WBFT 헤더 검증 로직

- [ ] **작업 3.3.1**: validate_header 구현
  - 파일: `crates/consensus/wbft/src/validation.rs`
  - 구현 내용:
    ```rust
    impl<H: BlockHeader> HeaderValidator<H> for WbftConsensus<ChainSpec> {
        fn validate_header(&self, header: &SealedHeader<H>) -> Result<(), ConsensusError> {
            // 1. Extra data 파싱
            let extra = WbftExtra::decode(header.extra_data())?;

            // 2. 검증자 세트 로드
            let validators = self.get_validators(header.number())?;

            // 3. Prepared seal 검증
            if let Some(seal) = &extra.prepared_seal {
                self.verify_aggregated_seal(seal, &validators, header, extra.round, SealType::Prepare)?;
            }

            // 4. Committed seal 검증
            if let Some(seal) = &extra.committed_seal {
                self.verify_aggregated_seal(seal, &validators, header, extra.round, SealType::Commit)?;
            }

            // 5. Quorum 확인
            self.check_quorum(seal, validators.quorum_size())?;

            Ok(())
        }
    }
    ```
  - 예상 시간: 3일

- [ ] **작업 3.3.2**: validate_header_against_parent 구현
  - 파일: `crates/consensus/wbft/src/validation.rs`
  - 구현 내용:
    ```rust
    fn validate_header_against_parent(
        &self,
        header: &SealedHeader<H>,
        parent: &SealedHeader<H>
    ) -> Result<(), ConsensusError> {
        // 기본 검증
        validate_against_parent_hash_number(header.header(), parent)?;
        validate_against_parent_timestamp(header.header(), parent.header())?;

        // 제안자 검증
        let extra = WbftExtra::decode(header.extra_data())?;
        let validators = self.get_validators(header.number())?;
        let proposer = self.extract_proposer(header)?;

        // 올바른 제안자인지 확인
        self.verify_proposer(proposer, &validators, extra.round)?;

        Ok(())
    }
    ```
  - 예상 시간: 2일

- [ ] **작업 3.3.3**: 보조 검증 함수들
  - 파일: `crates/consensus/wbft/src/validation.rs`
  - 구현 내용:
    ```rust
    impl WbftConsensus {
        fn verify_aggregated_seal(
            &self,
            seal: &WbftAggregatedSeal,
            validators: &ValidatorSet,
            header: &Header,
            round: u32,
            seal_type: SealType
        ) -> Result<()> { ... }

        fn check_quorum(&self, seal: &WbftAggregatedSeal, quorum: usize) -> Result<()> { ... }

        fn extract_proposer(&self, header: &Header) -> Result<Address> { ... }

        fn verify_proposer(&self, proposer: Address, validators: &ValidatorSet, round: u32) -> Result<()> { ... }
    }
    ```
  - 예상 시간: 2일

---

### 3.4 FullConsensus Trait 구현 ✅ 우선순위: P1

**목표**: 실행 후 검증

- [ ] **작업 3.4.1**: validate_block_post_execution 구현
  - 파일: `crates/consensus/wbft/src/validation.rs`
  - 구현 내용:
    ```rust
    impl<N: NodePrimitives> FullConsensus<N> for WbftConsensus<ChainSpec> {
        fn validate_block_post_execution(
            &self,
            block: &RecoveredBlock<N::Block>,
            result: &BlockExecutionResult<N::Receipt>
        ) -> Result<(), ConsensusError> {
            // 기본 post-execution 검증
            validate_block_post_execution(
                block,
                &self.chain_spec,
                &result.receipts,
                &result.requests
            )?;

            // Epoch 전환 검증 (해당시)
            if self.is_epoch_block(block.number()) {
                self.validate_epoch_transition(block, result)?;
            }

            Ok(())
        }
    }
    ```
  - 예상 시간: 2일

---

### 3.5 ChainSpec 통합 ✅ 우선순위: P0

**목표**: go-stablenet의 AnzeonConfig를 reth ChainSpec에 통합

**참조**: `genesis-integration-guide.md` 섹션 1-3

- [ ] **작업 3.5.1**: WbftChainConfig 구조체 정의 (AnzeonConfig 대응)
  - 파일: `crates/chainspec/src/wbft.rs` (신규)
  - 구현 내용:
    ```rust
    /// go-stablenet의 AnzeonConfig에 대응
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WbftChainConfig {
        pub wbft: WbftConfig,              // 합의 파라미터
        pub init: WbftInit,                // 초기 검증자 설정
        pub system_contracts: SystemContracts,  // 시스템 컨트랙트
    }

    /// go-stablenet의 WBFTConfig에 대응
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WbftConfig {
        pub request_timeout_seconds: u64,
        pub block_period_seconds: u64,
        pub proposer_policy: u64,  // 0=RoundRobin, 1=Sticky
        pub epoch_length: u64,
        pub max_request_timeout_seconds: Option<u64>,
    }

    /// go-stablenet의 WBFTInit에 대응
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WbftInit {
        pub validators: Vec<Address>,      // 초기 검증자 주소
        pub bls_public_keys: Vec<String>,  // "0x..." hex 형식
    }

    /// go-stablenet의 SystemContracts에 대응
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SystemContracts {
        pub gov_validator: SystemContract,
        pub native_coin_adapter: Option<SystemContract>,
        pub gov_minter: Option<SystemContract>,
        pub gov_master_minter: Option<SystemContract>,
        pub gov_council: Option<SystemContract>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SystemContract {
        pub address: Address,
        pub version: Option<String>,
        pub params: HashMap<String, String>,  // 예: gasTip
    }
    ```
  - 예상 시간: 2일
  - 검증: JSON 직렬화/역직렬화 테스트

- [ ] **작업 3.5.2**: 설정 검증 로직
  - 파일: `crates/chainspec/src/wbft.rs`
  - 구현 내용:
    ```rust
    impl WbftChainConfig {
        pub fn validate(&self) -> Result<(), String> {
            // 1. 검증자 수 = BLS 키 수
            if self.init.validators.len() != self.init.bls_public_keys.len() {
                return Err("Validator count mismatch".into());
            }

            // 2. 최소 1개 검증자
            if self.init.validators.is_empty() {
                return Err("At least one validator required".into());
            }

            // 3. BLS 키 형식 검증 (0x + 96 hex chars)
            for key in &self.init.bls_public_keys {
                if !key.starts_with("0x") || key.len() != 98 {
                    return Err(format!("Invalid BLS key: {}", key));
                }
            }

            // 4. Epoch 길이 > 0
            if self.wbft.epoch_length == 0 {
                return Err("Epoch length must be > 0".into());
            }

            Ok(())
        }

        pub fn initial_bls_public_keys(&self) -> Result<Vec<Vec<u8>>, String> {
            self.init.bls_public_keys.iter()
                .map(|s| hex::decode(s.trim_start_matches("0x"))
                    .map_err(|e| format!("Invalid hex: {}", e)))
                .collect()
        }
    }
    ```
  - 테스트:
    - `test_valid_config` - 정상 설정
    - `test_validator_bls_mismatch` - 검증자/키 불일치
    - `test_invalid_bls_format` - 잘못된 hex
    - `test_empty_validators` - 빈 검증자
  - 예상 시간: 2일

- [ ] **작업 3.5.3**: ChainSpec 확장
  - 파일: `crates/chainspec/src/spec.rs`
  - 구현 내용:
    ```rust
    pub struct ChainSpec {
        // 기존 필드들...

        /// WBFT consensus configuration (go-stablenet의 Anzeon)
        #[serde(skip_serializing_if = "Option::is_none")]
        pub wbft: Option<WbftChainConfig>,
    }

    impl ChainSpec {
        pub fn is_wbft(&self) -> bool {
            self.wbft.is_some()
        }

        pub fn wbft_config_at(&self, block_number: u64) -> Option<WbftConfig> {
            self.wbft.as_ref().map(|w| w.get_config(block_number))
        }
    }
    ```
  - 예상 시간: 1일
  - 검증: 기존 ChainSpec 역호환성 확인

- [ ] **작업 3.5.4**: Transitions 및 Upgrades 지원
  - 파일: `crates/chainspec/src/wbft.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct WbftTransition {
        pub block: U256,
        pub request_timeout_seconds: Option<u64>,
        pub block_period_seconds: Option<u64>,
        pub epoch_length: Option<u64>,
        pub proposer_policy: Option<u64>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SystemContractUpgrade {
        pub block: U256,
        pub gov_validator: Option<Address>,
        pub gov_council: Option<Address>,
        // ... 기타 컨트랙트
    }

    impl WbftChainConfig {
        pub fn get_config(&self, block_number: u64) -> WbftConfig {
            // transitions 적용하여 동적 설정 반환
        }

        pub fn get_system_contracts(&self, block_number: u64) -> SystemContracts {
            // upgrades 적용하여 활성 컨트랙트 주소 반환
        }
    }
    ```
  - 예상 시간: 2일

---

### 3.6 Genesis 초기화 ✅ 우선순위: P0

**목표**: Genesis 블록에 WBFT Extra data 자동 생성 (go-stablenet 호환)

**참조**: `genesis-integration-guide.md` 섹션 4-6

- [ ] **작업 3.6.1**: create_initial_extra_data 함수
  - 파일: `crates/consensus/wbft/src/genesis.rs`
  - 구현 내용 (go-stablenet의 CreateInitialExtraData 대응):
    ```rust
    /// Genesis extra data 생성 (go-stablenet 호환)
    pub fn create_initial_extra_data(
        config: &WbftChainConfig
    ) -> Result<Bytes, String> {
        // 1. 설정 검증
        config.validate()?;

        // 2. EpochInfo 생성
        let epoch_info = create_initial_epoch_info(config)?;

        // 3. Gas tip 파싱 (SystemContract params에서)
        let gas_tip = config.system_contracts.gov_validator.params
            .get("gasTip")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1_000_000_000);  // 기본 1 Gwei

        // 4. WBFTExtra 구조체 생성
        let extra = WbftExtra {
            vanity_data: [0u8; 32],
            randao_reveal: vec![],
            prev_round: 0,
            prev_prepared_seal: None,
            prev_committed_seal: None,
            round: 0,
            prepared_seal: None,
            committed_seal: None,
            gas_tip: U256::from(gas_tip),
            epoch_info: Some(epoch_info),
        };

        // 5. RLP 인코딩 (go-stablenet과 동일)
        let mut encoded = Vec::new();
        extra.encode(&mut encoded);
        Ok(Bytes::from(encoded))
    }
    ```
  - 예상 시간: 2일
  - 검증: go-stablenet 생성 extra data와 비교

- [ ] **작업 3.6.2**: create_initial_epoch_info 함수
  - 파일: `crates/consensus/wbft/src/genesis.rs`
  - 구현 내용:
    ```rust
    fn create_initial_epoch_info(
        config: &WbftChainConfig
    ) -> Result<EpochInfo, String> {
        let mut candidates = Vec::new();
        let mut validators = Vec::new();
        let bls_keys = config.initial_bls_public_keys()?;

        // 모든 초기 검증자를 후보자로 추가
        for (i, &addr) in config.init.validators.iter().enumerate() {
            candidates.push(Candidate {
                addr,
                diligence: 950_000,  // 95% (단위: 10^-6)
            });
            validators.push(i as u32);
        }

        Ok(EpochInfo {
            candidates,
            validators,
            bls_public_keys: bls_keys,
        })
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 3.6.3**: Genesis::to_block WBFT 통합
  - 파일: `crates/chainspec/src/genesis.rs`
  - 수정 내용:
    ```rust
    impl Genesis {
        pub fn to_block(
            &self,
            wbft_config: Option<&WbftChainConfig>
        ) -> Result<SealedBlock, String> {
            // WBFT extra data 자동 생성
            let extra_data = if let Some(config) = wbft_config {
                create_initial_extra_data(config)?
            } else {
                self.extra_data.clone()
            };

            // 헤더 생성 (extra_data 사용)
            let header = Header {
                // ... 기타 필드
                extra_data,
            };

            Ok(SealedBlock::new(header, BlockBody::default()))
        }
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 3.6.4**: Genesis JSON 파싱 및 검증
  - 파일: `crates/chainspec/src/spec.rs`
  - Genesis JSON 형식 (go-stablenet 호환):
    ```json
    {
      "config": {
        "chainId": 1234,
        "wbft": {
          "wbft": {
            "requestTimeoutSeconds": 2,
            "blockPeriodSeconds": 1,
            "proposerPolicy": 0,
            "epochLength": 10
          },
          "init": {
            "validators": ["0x1234...", "0xabcd..."],
            "blsPublicKeys": ["0xaec4...", "0xb1ae..."]
          },
          "systemContracts": {
            "govValidator": {
              "address": "0x0000...0400",
              "version": "v1",
              "params": {
                "gasTip": "1000000000"
              }
            }
          }
        }
      },
      "extraData": "0x"
    }
    ```
  - ChainSpec::from_json() 확장:
    ```rust
    impl ChainSpec {
        pub fn from_json(json: &str) -> Result<Self, Error> {
            let mut spec: ChainSpec = serde_json::from_str(json)?;

            // WBFT 설정 검증
            if let Some(ref wbft) = spec.wbft {
                wbft.validate()?;
            }

            Ok(spec)
        }
    }
    ```
  - 예상 시간: 2일
  - 테스트:
    - `test_parse_wbft_genesis_json`
    - `test_genesis_extra_data_generation`
    - `test_genesis_block_hash_matches_gostable`

- [ ] **작업 3.6.5**: Node 초기화 통합
  - 파일: Node builder (위치는 reth 구조에 따름)
  - 구현 내용:
    ```rust
    // Genesis 로드 및 블록 생성
    let chain_spec = ChainSpec::from_json(&genesis_json)?;
    let genesis_block = if let Some(ref wbft) = chain_spec.wbft {
        chain_spec.genesis.to_block(Some(wbft))?
    } else {
        chain_spec.genesis.to_block(None)?
    };

    // DB에 커밋
    commit_genesis(db, genesis_block)?;
    ```
  - 예상 시간: 2일

---

## Phase 4: 네트워크 및 고급 기능 (3-4주)

### 4.1 네트워크 프로토콜 ✅ 우선순위: P0

**목표**: P2P 합의 메시지 전송

- [ ] **작업 4.1.1**: WBFT 프로토콜 정의
  - 파일: `crates/consensus/wbft/src/network/protocol.rs`
  - 구현 내용:
    ```rust
    pub const WBFT_PROTOCOL_ID: &str = "wbft";
    pub const WBFT_VERSION: u8 = 1;

    pub const MSG_PREPREPARE: u8 = 0x12;
    pub const MSG_PREPARE: u8 = 0x13;
    pub const MSG_COMMIT: u8 = 0x14;
    pub const MSG_ROUNDCHANGE: u8 = 0x15;

    #[derive(Debug)]
    pub struct WbftProtocol;

    impl RlpxSubProtocol for WbftProtocol {
        fn protocol_name(&self) -> &str { WBFT_PROTOCOL_ID }
        fn version(&self) -> u8 { WBFT_VERSION }
        fn supported_messages(&self) -> Vec<u8> {
            vec![MSG_PREPREPARE, MSG_PREPARE, MSG_COMMIT, MSG_ROUNDCHANGE]
        }
    }
    ```
  - 예상 시간: 2일

- [ ] **작업 4.1.2**: 메시지 핸들러
  - 파일: `crates/consensus/wbft/src/network/handler.rs`
  - 구현 내용:
    ```rust
    pub struct WbftProtocolHandler {
        core: Arc<Mutex<Core>>,
        known_messages: LruCache<B256, ()>,
    }

    impl ProtocolHandler for WbftProtocolHandler {
        async fn on_message(&mut self, peer_id: PeerId, code: u8, data: Bytes) -> Result<()> {
            // 1. 중복 확인
            let hash = keccak256(&data);
            if self.known_messages.contains(&hash) {
                return Ok(());
            }

            // 2. 메시지 디코딩
            let msg = match code {
                MSG_PREPREPARE => decode_preprepare(&data)?,
                MSG_PREPARE => decode_prepare(&data)?,
                MSG_COMMIT => decode_commit(&data)?,
                MSG_ROUNDCHANGE => decode_roundchange(&data)?,
                _ => return Err(...),
            };

            // 3. Core로 전달
            self.core.lock().await.handle_message(code, msg).await?;

            // 4. 캐시에 추가
            self.known_messages.put(hash, ());

            Ok(())
        }
    }
    ```
  - 예상 시간: 3일

- [ ] **작업 4.1.3**: 브로드캐스트 및 Gossip
  - 파일: `crates/consensus/wbft/src/network/broadcast.rs`
  - 구현 내용:
    ```rust
    pub struct Broadcaster {
        network: NetworkHandle,
    }

    impl Broadcaster {
        pub async fn broadcast(&self, validators: &ValidatorSet, code: u8, payload: &[u8]) -> Result<()> {
            // 모든 검증자 피어에게 전송 (자신 포함)
            self.gossip(validators, code, payload).await?;
            // 자신에게도 전송 (이벤트 채널)
        }

        pub async fn gossip(&self, validators: &ValidatorSet, code: u8, payload: &[u8]) -> Result<()> {
            // 다른 검증자 피어에게만 전송
            let peers = self.get_validator_peers(validators).await?;
            for peer in peers {
                self.network.send_message(peer, code, payload.to_vec()).await?;
            }
        }
    }
    ```
  - 예상 시간: 2일

---

### 4.2 Backend 구현 ✅ 우선순위: P0

**목표**: Core와 블록체인 연결

- [ ] **작업 4.2.1**: Backend trait 정의
  - 파일: `crates/consensus/wbft/src/backend/mod.rs`
  - 구현 내용:
    ```rust
    #[async_trait]
    pub trait Backend: Send + Sync {
        fn address(&self) -> Address;
        fn validators(&self, proposal: &SealedBlock) -> Arc<dyn ValidatorSet>;
        async fn broadcast(&self, validators: &ValidatorSet, code: u8, payload: &[u8]) -> Result<()>;
        async fn gossip(&self, validators: &ValidatorSet, code: u8, payload: &[u8]) -> Result<()>;
        async fn commit(&self, proposal: SealedBlock, seals: Seals) -> Result<()>;
        async fn verify(&self, proposal: &SealedBlock) -> Result<Duration>;
        fn sign(&self, data: &[u8]) -> Vec<u8>;
        fn sign_bls(&self, data: &[u8]) -> Vec<u8>;
        fn check_signature(&self, data: &[u8], addr: Address, sig: &[u8]) -> Result<()>;
    }
    ```
  - 예상 시간: 2일

- [ ] **작업 4.2.2**: Backend 구현체
  - 파일: `crates/consensus/wbft/src/backend/implementation.rs`
  - 구현 내용:
    ```rust
    pub struct WbftBackend {
        address: Address,
        secret_key: SecretKey,
        bls_secret_key: bls::SecretKey,
        blockchain: Arc<dyn BlockReader>,
        broadcaster: Arc<Broadcaster>,
        validator_provider: Arc<dyn ValidatorProvider>,
    }

    #[async_trait]
    impl Backend for WbftBackend {
        // trait 메서드 구현
    }
    ```
  - 예상 시간: 3일

---

### 4.3 블록 씰링 (Sealing) ✅ 우선순위: P1

**목표**: 합의 완료 후 블록에 서명 추가

- [ ] **작업 4.3.1**: Sealer 구조체
  - 파일: `crates/consensus/wbft/src/sealer.rs`
  - 구현 내용:
    ```rust
    pub struct WbftSealer {
        core: Arc<Mutex<Core>>,
        backend: Arc<dyn Backend>,
    }

    impl WbftSealer {
        pub async fn seal_block(&self, mut block: SealedBlock) -> Result<SealedBlock> {
            // 1. 블록을 Core에 제출
            let (prepared_seal, committed_seal) = self.core.lock().await
                .seal_request(block.clone())
                .await?;

            // 2. Extra data에 seals 추가
            let mut extra = WbftExtra::decode(block.header().extra_data())?;
            extra.prepared_seal = Some(prepared_seal);
            extra.committed_seal = Some(committed_seal);

            // 3. 헤더 업데이트
            let new_extra_data = extra.encode()?;
            block.set_extra_data(new_extra_data);

            Ok(block)
        }
    }
    ```
  - 예상 시간: 2일

- [ ] **작업 4.3.2**: 합의 대기 메커니즘
  - 파일: `crates/consensus/wbft/src/core/seal.rs`
  - 구현 내용:
    ```rust
    impl Core {
        pub async fn seal_request(&mut self, block: SealedBlock) -> Result<(WbftAggregatedSeal, WbftAggregatedSeal)> {
            // 1. 합의 시작
            self.send_request(block).await?;

            // 2. 완료 대기 (채널)
            let seals = self.seal_result_rx.recv().await?;

            Ok(seals)
        }
    }
    ```
  - 예상 시간: 2일

---

### 4.4 Epoch 관리 ✅ 우선순위: P1

**목표**: 검증자 세트 주기적 업데이트

- [ ] **작업 4.4.1**: EpochInfo 타입
  - 파일: `crates/consensus/wbft/src/epoch.rs`
  - 구현 내용:
    ```rust
    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct EpochInfo {
        pub candidates: Vec<Candidate>,
        pub validators: Vec<u32>,
        pub bls_public_keys: Vec<Vec<u8>>,
    }

    #[derive(Debug, Clone, RlpEncodable, RlpDecodable)]
    pub struct Candidate {
        pub addr: Address,
        pub diligence: u64,
    }
    ```
  - 예상 시간: 1일

- [ ] **작업 4.4.2**: Epoch 전환 로직
  - 파일: `crates/consensus/wbft/src/epoch.rs`
  - 구현 내용:
    ```rust
    pub struct EpochManager {
        config: WbftConfig,
    }

    impl EpochManager {
        pub fn is_epoch_block(&self, number: u64) -> bool {
            number > 0 && number % self.config.epoch == 0
        }

        pub fn load_epoch_info(&self, header: &Header) -> Result<Option<EpochInfo>> {
            if !self.is_epoch_block(header.number()) {
                return Ok(None);
            }

            let extra = WbftExtra::decode(header.extra_data())?;
            Ok(extra.epoch_info)
        }
    }
    ```
  - 예상 시간: 2일

---

### 4.5 시스템 컨트랙트 통합 ✅ 우선순위: P1

**목표**: 온체인 검증자 관리

- [ ] **작업 4.5.1**: GovValidator 컨트랙트 인터페이스
  - 파일: `crates/consensus/wbft/src/contracts/gov_validator.rs`
  - 구현 내용:
    ```rust
    pub struct GovValidator {
        address: Address,
    }

    impl GovValidator {
        pub fn read_validators(&self, state: &dyn StateProvider) -> Result<Vec<Address>> {
            // SLOT_VALIDATOR_VALIDATORS 읽기
        }

        pub fn read_bls_public_key(&self, state: &dyn StateProvider, validator: Address) -> Result<Vec<u8>> {
            // SLOT_VALIDATOR_VALIDATOR_TO_BLS_KEY 읽기
        }

        pub fn read_gas_tip(&self, state: &dyn StateProvider) -> Result<U256> {
            // SLOT_VALIDATOR_GAS_TIP 읽기
        }
    }
    ```
  - 예상 시간: 3일

- [ ] **작업 4.5.2**: ValidatorProvider 구현
  - 파일: `crates/consensus/wbft/src/validator/provider.rs`
  - 구현 내용:
    ```rust
    pub trait ValidatorProvider: Send + Sync {
        fn get_validators(&self, block_number: u64) -> Result<Arc<dyn ValidatorSet>>;
    }

    pub struct ContractValidatorProvider {
        gov_validator: GovValidator,
        blockchain: Arc<dyn BlockReader>,
        cache: RwLock<LruCache<u64, Arc<dyn ValidatorSet>>>,
    }

    impl ValidatorProvider for ContractValidatorProvider {
        fn get_validators(&self, block_number: u64) -> Result<Arc<dyn ValidatorSet>> {
            // 1. 캐시 확인
            // 2. Epoch 블록 찾기
            // 3. EpochInfo 로드 또는 컨트랙트 조회
            // 4. ValidatorSet 생성 및 캐시
        }
    }
    ```
  - 예상 시간: 3일

- [ ] **작업 4.5.3**: 시스템 컨트랙트 업그레이드
  - 파일: `crates/consensus/wbft/src/contracts/upgrade.rs`
  - 구현 내용:
    ```rust
    pub struct SystemContractUpgrade {
        pub block: u64,
        pub gov_validator: Option<Address>,
        pub gov_council: Option<Address>,
        // ... 기타 컨트랙트
    }

    impl WbftConfig {
        pub fn get_system_contracts(&self, block_number: u64) -> SystemContracts {
            // transitions 순회하며 활성 주소 결정
        }
    }
    ```
  - 예상 시간: 2일

---

## Phase 5: 테스트 및 최적화 (2-3주)

### 5.1 단위 테스트 ✅ 우선순위: P0

**목표**: 모든 컴포넌트 단위 테스트

- [ ] **작업 5.1.1**: BLS 테스트
  - 위치: `crates/consensus/wbft/src/bls/tests.rs`
  - 테스트 케이스:
    - `test_key_generation`
    - `test_sign_verify`
    - `test_aggregate_3_signatures`
    - `test_aggregate_7_signatures`
    - `test_sealer_set_bitmap`
  - 예상 시간: 2일

- [ ] **작업 5.1.2**: 메시지 인코딩 테스트
  - 위치: `crates/consensus/wbft/src/messages/tests.rs`
  - 테스트 케이스:
    - `test_preprepare_rlp_roundtrip`
    - `test_prepare_rlp_roundtrip`
    - `test_commit_rlp_roundtrip`
    - `test_roundchange_rlp_roundtrip`
  - 예상 시간: 1일

- [ ] **작업 5.1.3**: ValidatorSet 테스트
  - 위치: `crates/consensus/wbft/src/validator/tests.rs`
  - 테스트 케이스:
    - `test_quorum_size` - 3,4,7,10 검증자
    - `test_round_robin_proposer`
    - `test_sticky_proposer`
    - `test_f_calculation`
  - 예상 시간: 2일

- [ ] **작업 5.1.4**: 헤더 검증 테스트
  - 위치: `crates/consensus/wbft/src/validation/tests.rs`
  - 테스트 케이스:
    - `test_valid_header_with_seals`
    - `test_invalid_signature`
    - `test_insufficient_quorum`
    - `test_wrong_proposer`
  - 예상 시간: 2일

---

### 5.2 통합 테스트 ✅ 우선순위: P0

**목표**: 엔드투엔드 시나리오 테스트

- [ ] **작업 5.2.1**: 단일 검증자 테스트
  - 위치: `crates/consensus/wbft/tests/single_validator.rs`
  - 시나리오:
    - 블록 생성 및 씰링
    - 헤더 검증
  - 예상 시간: 2일

- [ ] **작업 5.2.2**: 다중 검증자 합의 테스트
  - 위치: `crates/consensus/wbft/tests/consensus.rs`
  - 시나리오:
    - 3 검증자 정상 합의
    - 4 검증자 정상 합의
    - 7 검증자 정상 합의
  - 예상 시간: 3일

- [ ] **작업 5.2.3**: 라운드 변경 테스트
  - 위치: `crates/consensus/wbft/tests/roundchange.rs`
  - 시나리오:
    - 타임아웃 발생 및 라운드 변경
    - 라운드 변경 후 합의 완료
    - 여러 라운드 변경
  - 예상 시간: 3일

- [ ] **작업 5.2.4**: Epoch 전환 테스트
  - 위치: `crates/consensus/wbft/tests/epoch.rs`
  - 시나리오:
    - Epoch 블록 생성
    - 검증자 세트 변경
    - 새로운 검증자로 합의
  - 예상 시간: 2일

---

### 5.3 성능 벤치마크 ✅ 우선순위: P2

**목표**: 성능 측정 및 최적화

- [ ] **작업 5.3.1**: BLS 서명 벤치마크
  - 위치: `crates/consensus/wbft/benches/bls.rs`
  - 측정 항목:
    - 개별 서명 생성
    - 서명 검증
    - 서명 집계 (3, 7, 13 서명)
  - 예상 시간: 2일

- [ ] **작업 5.3.2**: 메시지 처리 벤치마크
  - 위치: `crates/consensus/wbft/benches/messages.rs`
  - 측정 항목:
    - 메시지 인코딩/디코딩
    - 메시지 처리 처리량
  - 예상 시간: 1일

- [ ] **작업 5.3.3**: 합의 지연시간 벤치마크
  - 위치: `crates/consensus/wbft/benches/consensus.rs`
  - 측정 항목:
    - 블록 씰링 시간 (검증자 수별)
    - 라운드 변경 시간
  - 예상 시간: 2일

---

### 5.4 문서화 ✅ 우선순위: P2

**목표**: 포괄적인 문서 작성

- [ ] **작업 5.4.1**: 아키텍처 문서
  - 위치: `docs/consensus/wbft/architecture.md`
  - 내용:
    - WBFT 프로토콜 개요
    - 상태 머신 다이어그램
    - 컴포넌트 구조
  - 예상 시간: 2일

- [ ] **작업 5.4.2**: 통합 가이드
  - 위치: `docs/consensus/wbft/integration.md`
  - 내용:
    - reth에 WBFT 추가 방법
    - 설정 파일 예제
    - 검증자 노드 설정
  - 예상 시간: 2일

- [ ] **작업 5.4.3**: API 레퍼런스
  - 위치: `docs/consensus/wbft/api.md`
  - 내용:
    - RPC 메서드
    - 설정 옵션
    - 메시지 형식
  - 예상 시간: 1일

- [ ] **작업 5.4.4**: 코드 문서화
  - 모든 public API에 rustdoc 추가
  - 예제 코드 작성
  - 예상 시간: 3일

---

## 추가 작업 (선택적)

### 6.1 모니터링 및 메트릭 🔵 우선순위: P3

- [ ] **작업 6.1.1**: 메트릭 수집
  - 파일: `crates/consensus/wbft/src/metrics.rs`
  - 메트릭:
    - `wbft_round_duration`
    - `wbft_round_changes_total`
    - `wbft_messages_received_total`
  - 예상 시간: 2일

- [ ] **작업 6.1.2**: 디버깅 RPC API
  - 파일: `crates/consensus/wbft/src/rpc.rs`
  - 메서드:
    - `wbft_getValidators`
    - `wbft_getCurrentRound`
    - `wbft_getMessageStats`
  - 예상 시간: 3일

---

### 6.2 최적화 🔵 우선순위: P3

- [ ] **작업 6.2.1**: 메모리 최적화
  - 메시지 풀 크기 제한
  - 오래된 메시지 정리
  - 예상 시간: 2일

- [ ] **작업 6.2.2**: 네트워크 최적화
  - 메시지 배칭
  - 압축 (선택적)
  - 예상 시간: 2일

---

## 체크리스트 범례

- **✅ 우선순위 P0**: 핵심 기능, 필수
- **✅ 우선순위 P1**: 중요 기능
- **✅ 우선순위 P2**: 보완 기능
- **🔵 우선순위 P3**: 선택적 기능

---

## 마일스톤 및 완료 기준

### Milestone 1: 기초 완료 (3주 차)
- [ ] BLS 서명 및 집계 작동
- [ ] 모든 메시지 타입 인코딩/디코딩
- [ ] ValidatorSet 및 제안자 선출
- [ ] 단위 테스트 80% 커버리지

### Milestone 2: 합의 프로토콜 완료 (7주 차)
- [ ] 3단계 합의 작동
- [ ] 라운드 변경 작동
- [ ] 단일/다중 검증자 통합 테스트 통과

### Milestone 3: reth 통합 완료 (11주 차)
- [ ] Consensus/HeaderValidator/FullConsensus trait 구현
- [ ] ChainSpec 통합
- [ ] Genesis 초기화
- [ ] 블록 검증 성공

### Milestone 4: 프로덕션 준비 (15주 차)
- [ ] 네트워크 프로토콜 작동
- [ ] 시스템 컨트랙트 통합
- [ ] Epoch 전환 작동
- [ ] 모든 테스트 통과
- [ ] 문서 완성

---

## 일일 진행 추적

**권장 사용법**:
1. 매일 시작시 진행할 작업 선택
2. 완료시 체크박스 표시
3. 문제 발생시 작업 항목에 메모 추가
4. 주간 리뷰로 진행 상황 평가

**예시**:
```
- [x] 작업 1.1.1: WBFT 크레이트 생성 (2025-XX-XX 완료)
  - 메모: Cargo.toml에 blst 의존성 추가 완료
- [ ] 작업 1.1.2: 워크스페이스 등록 (진행 중)
  - 블로커: workspace Cargo.toml 위치 확인 필요
```

---

## 리스크 및 완화 전략

| 리스크 | 가능성 | 영향 | 완화 전략 |
|--------|-------|------|----------|
| BLS 라이브러리 호환성 문제 | 중 | 높음 | blst 라이브러리 초기 검증, 대안 준비 |
| reth 아키텍처 변경 | 중 | 중간 | 정기적인 upstream 추적, 버전 고정 |
| 성능 목표 미달성 | 낮 | 중간 | 초기 벤치마킹, 점진적 최적화 |
| 네트워크 통합 복잡도 | 높음 | 높음 | Optimism 참조 구현 분석, 단계적 구현 |

---

## 성공 기준

**기능 완성도**:
- [x] Phase 1-4 모든 P0 작업 완료
- [ ] 단위 테스트 커버리지 ≥ 80%
- [ ] 통합 테스트 모든 시나리오 통과

**성능**:
- [ ] 4 검증자 블록 타임 ≤ 2초
- [ ] BLS 서명 집계 ≤ 10ms (7 서명)
- [ ] 메시지 처리 ≥ 500 msg/s

**품질**:
- [ ] cargo clippy 경고 0개
- [ ] 모든 public API rustdoc 완료
- [ ] 통합 가이드 및 예제 완성

---

## 다음 단계

1. **Phase 1 시작**: 작업 1.1.1부터 순차적으로 진행
2. **일일 스탠드업**: 진행 상황 및 블로커 공유
3. **주간 리뷰**: 마일스톤 진행도 평가
4. **코드 리뷰**: 각 Phase 완료시 전체 리뷰

---

**마지막 업데이트**: 2025-XX-XX
**다음 리뷰**: Phase 1 완료시
