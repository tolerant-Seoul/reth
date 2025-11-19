# WBFT Consensus Porting to Reth - Feature List

## Overview

이 문서는 go-stablenet의 WBFT (Wemix Byzantine Fault Tolerant) 합의 알고리즘을 reth 프로젝트로 포팅하기 위한 전체 기능 목록을 정리합니다.

**Source**: go-stablenet WBFT consensus (~51 Go files, 3,400+ lines)
**Target**: reth consensus architecture (Rust, trait-based modular design)

---

## 1. 핵심 합의 프로토콜 (Core Consensus Protocol)

### 1.1 WBFT State Machine
**설명**: 3단계 커밋 프로토콜을 구현하는 핵심 상태 머신

**주요 컴포넌트**:
- **상태 정의** (StateAcceptRequest, StatePreprepared, StatePrepared, StateCommitted)
- **상태 전이 로직** (상태간 전환 규칙 및 검증)
- **타이머 관리** (라운드 타임아웃, 지수 백오프)
- **이벤트 루프** (메시지 처리 및 상태 업데이트)

**Go 구현 위치**: `consensus/wbft/core/core.go`, `consensus/wbft/core/types.go`
**Rust 대상**: `crates/consensus/wbft/src/state_machine.rs`

**기술적 고려사항**:
- Go의 채널 기반 이벤트 루프 → Rust의 tokio async/await 모델로 전환
- 타이머 관리: Go의 time.Timer → tokio::time::sleep/interval
- 상태 동기화: Go의 sync.RWMutex → Rust의 RwLock/Mutex

---

### 1.2 메시지 타입 및 프로토콜 (Message Types & Protocol)

#### 1.2.1 PRE-PREPARE 메시지
**설명**: 제안자가 블록을 제안하는 메시지

**필드**:
- `view`: View{Round, Sequence} - 현재 라운드 및 블록 번호
- `proposal`: Block - 제안 블록
- `round_change_justification`: []RoundChangeMessage - 라운드 변경 정당화

**Go 구현**: `consensus/wbft/messages/preprepare.go`, `consensus/wbft/core/preprepare.go`
**Rust 대상**: `crates/consensus/wbft/src/messages/preprepare.rs`

**구현 요구사항**:
- RLP 인코딩/디코딩
- 메시지 검증 로직
- 정당화 검증 (라운드 변경시)

#### 1.2.2 PREPARE 메시지
**설명**: 검증자가 PRE-PREPARE를 승인하는 메시지

**필드**:
- `view`: View - 현재 뷰
- `digest`: Hash - 블록 해시
- `bls_prepare_seal`: []byte - BLS 서명 (96 bytes)

**Go 구현**: `consensus/wbft/messages/prepare.go`, `consensus/wbft/core/prepare.go`
**Rust 대상**: `crates/consensus/wbft/src/messages/prepare.rs`

**구현 요구사항**:
- BLS 서명 생성 및 검증
- Quorum 수집 (ceil(2n/3))
- 메시지 중복 방지

#### 1.2.3 COMMIT 메시지
**설명**: 검증자가 최종 커밋하는 메시지

**필드**:
- `view`: View - 현재 뷰
- `digest`: Hash - 블록 해시
- `bls_commit_seal`: []byte - BLS 서명 (96 bytes)

**Go 구현**: `consensus/wbft/messages/commit.go`, `consensus/wbft/core/commit.go`
**Rust 대상**: `crates/consensus/wbft/src/messages/commit.rs`

**구현 요구사항**:
- BLS 서명 집계
- Quorum 확인 후 블록 완료
- 집계된 서명을 블록 헤더에 저장

#### 1.2.4 ROUND-CHANGE 메시지
**설명**: 타임아웃 발생시 라운드를 변경하는 메시지

**필드**:
- `view`: View - 목표 뷰 (round+1)
- `prepared_round`: *big.Int - 준비된 라운드 (있는 경우)
- `prepared_digest`: Hash - 준비된 블록 해시
- `justification`: []PrepareMessage - PREPARE 메시지 증명

**Go 구현**: `consensus/wbft/messages/roundchange.go`, `consensus/wbft/core/roundchange.go`
**Rust 대상**: `crates/consensus/wbft/src/messages/roundchange.rs`

**구현 요구사항**:
- F+1 메시지 수집시 라운드 이동
- Quorum 수집시 새 제안자 선출
- 정당화 검증 (prepared block이 있는 경우)

---

### 1.3 메시지 전송 및 네트워킹

**설명**: P2P 네트워크를 통한 합의 메시지 전파

**주요 기능**:
- **Broadcast**: 모든 검증자에게 메시지 전송 (자신 포함)
- **Gossip**: 다른 검증자에게만 메시지 전송
- **메시지 중복 제거**: LRU 캐시로 이미 처리한 메시지 필터링
- **백로그 관리**: 미래 블록/라운드 메시지 버퍼링

**Go 구현**: `consensus/wbft/backend/handler.go`, `consensus/wbft/core/backlog.go`
**Rust 대상**: `crates/consensus/wbft/src/network.rs`

**reth 통합 지점**:
- `reth-network` 사용하여 P2P 메시지 전송
- 새로운 프로토콜 메시지 타입 정의 (0x12-0x15)
- `NetworkHandle`을 통한 브로드캐스트

**기술적 과제**:
- Go의 `event.TypeMux` → Rust의 `tokio::sync::broadcast` 채널
- Gossip 프로토콜 통합 (devp2p 확장)

---

## 2. BLS 서명 및 집계 (BLS Signatures & Aggregation)

### 2.1 BLS 서명 생성 및 검증

**설명**: BLS12-381 곡선을 사용한 서명 체계

**주요 기능**:
- **개별 서명**: `Sign(secretKey, message) -> signature (96 bytes)`
- **서명 검증**: `Verify(publicKey, message, signature) -> bool`
- **메시지 해싱**: `PrepareSeal(header, round, sealType) -> hash`

**Go 구현**: `crypto/bls` 패키지, `consensus/wbft/core/extraseal.go`
**Rust 대상**: `crates/consensus/wbft/src/bls.rs`

**Rust 크레이트 후보**:
- `blst` - 고성능 BLS12-381 구현 (Ethereum 공식)
- `bls-signatures` - Alternative BLS 라이브러리

**구현 요구사항**:
- BLS 키 생성 및 관리
- 서명 직렬화/역직렬화 (압축 형식)
- 메시지 해싱 (Keccak256)

---

### 2.2 BLS 서명 집계

**설명**: 여러 검증자의 서명을 하나로 결합

**주요 기능**:
- **집계**: `AggregateSignatures([]signature) -> aggregated_signature (96 bytes)`
- **집계 검증**: `VerifyAggregated([]publicKey, message, aggregated_sig) -> bool`
- **Sealer Set**: 비트맵으로 서명자 인덱스 저장

**Go 구현**: `consensus/wbft/core/extraseal.go`
**Rust 대상**: `crates/consensus/wbft/src/bls/aggregation.rs`

**데이터 구조**:
```rust
pub struct WBFTAggregatedSeal {
    pub bitmap: Vec<u8>,          // 서명자 비트맵
    pub signature: [u8; 96],       // 집계된 BLS 서명
}
```

**구현 요구사항**:
- 비트맵 생성 및 파싱
- 서명 집계 알고리즘
- 집계된 서명 검증 (공개키 목록 필요)

---

### 2.3 블록 헤더 Extra Data

**설명**: 블록 헤더에 WBFT 합의 데이터 저장

**구조**:
```rust
pub struct WBFTExtra {
    pub vanity_data: Vec<u8>,           // 32 bytes vanity
    pub randao_reveal: Vec<u8>,         // BLS 서명 (96 bytes)
    pub prev_round: u32,
    pub prev_prepared_seal: Option<WBFTAggregatedSeal>,
    pub prev_committed_seal: Option<WBFTAggregatedSeal>,
    pub round: u32,
    pub prepared_seal: Option<WBFTAggregatedSeal>,
    pub committed_seal: Option<WBFTAggregatedSeal>,
    pub gas_tip: U256,
    pub epoch_info: Option<EpochInfo>,
}
```

**Go 구현**: `core/types/wbft_extra.go`
**Rust 대상**: `crates/consensus/wbft/src/header.rs`

**구현 요구사항**:
- RLP 인코딩/디코딩
- 헤더 파싱 및 검증
- Extra data 크기 제한 처리

---

## 3. 검증자 관리 (Validator Management)

### 3.1 ValidatorSet 인터페이스

**설명**: 검증자 세트 관리 및 제안자 선출

**주요 기능**:
- **제안자 계산**: `CalcProposer(lastProposer, round) -> Validator`
- **Quorum 계산**: `QuorumSize() -> int` (ceil(2n/3))
- **Byzantine tolerance**: `F() -> float64` ((n-1)/3)
- **검증자 검색**: `GetByAddress(address) -> (index, Validator)`

**Go 구현**: `consensus/wbft/validator/default.go`
**Rust 대상**: `crates/consensus/wbft/src/validator/set.rs`

**데이터 구조**:
```rust
pub trait ValidatorSet {
    fn calc_proposer(&mut self, last_proposer: Address, round: u64);
    fn size(&self) -> usize;
    fn list(&self) -> &[Validator];
    fn get_by_index(&self, index: u64) -> Option<&Validator>;
    fn get_by_address(&self, addr: Address) -> Option<(usize, &Validator)>;
    fn get_proposer(&self) -> &Validator;
    fn is_proposer(&self, addr: Address) -> bool;
    fn f(&self) -> f64;
    fn quorum_size(&self) -> usize;
}
```

---

### 3.2 제안자 선출 정책 (Proposer Policy)

#### 3.2.1 Round Robin
**설명**: 라운드마다 다음 검증자를 제안자로 선출

**알고리즘**:
```rust
fn round_robin_proposer(val_set: &ValidatorSet, proposer: Address, round: u64) -> &Validator {
    let seed = calc_seed(val_set, proposer, round) + 1;
    let pick = seed % val_set.size() as u64;
    val_set.get_by_index(pick).unwrap()
}
```

#### 3.2.2 Sticky
**설명**: 라운드 변경 전까지 동일 제안자 유지

**알고리즘**:
```rust
fn sticky_proposer(val_set: &ValidatorSet, proposer: Address, round: u64) -> &Validator {
    let seed = calc_seed(val_set, proposer, round);  // No +1
    let pick = seed % val_set.size() as u64;
    val_set.get_by_index(pick).unwrap()
}
```

**Go 구현**: `consensus/wbft/validator/default.go`
**Rust 대상**: `crates/consensus/wbft/src/validator/policy.rs`

---

### 3.3 Epoch 기반 검증자 세트

**설명**: 일정 블록 주기마다 검증자 세트 변경

**데이터 구조**:
```rust
pub struct EpochInfo {
    pub candidates: Vec<Candidate>,      // 모든 후보자
    pub validators: Vec<u32>,            // 활성 검증자 인덱스
    pub bls_public_keys: Vec<Vec<u8>>,   // BLS 공개키 목록
}

pub struct Candidate {
    pub addr: Address,
    pub diligence: u64,  // 성실도 점수 (단위: 10^-6)
}
```

**주요 기능**:
- Epoch 전환시 새로운 검증자 세트 로드
- 마지막 블록 헤더에 다음 Epoch 정보 저장
- Diligence 기반 검증자 선출

**Go 구현**: `core/types/epoch_info.go`, `consensus/wbft/config.go`
**Rust 대상**: `crates/consensus/wbft/src/epoch.rs`

---

## 4. 시스템 컨트랙트 통합 (System Contracts Integration)

### 4.1 GovValidator 컨트랙트

**설명**: 온체인 검증자 관리 컨트랙트

**주요 기능**:
- **검증자-운영자 매핑**: 검증자 주소 ↔ 운영자 주소
- **BLS 공개키 저장**: 각 검증자의 BLS 공개키
- **Gas Tip 거버넌스**: 온체인 투표로 gas tip 결정
- **BLS PoP 검증**: Proof of Possession 검증

**스토리지 슬롯**:
```rust
const SLOT_VALIDATOR_BLS_POP: &str = "0x32";
const SLOT_VALIDATOR_VALIDATORS: &str = "0x33";
const SLOT_VALIDATOR_VALIDATOR_TO_OPERATOR: &str = "0x35";
const SLOT_VALIDATOR_OPERATOR_TO_VALIDATOR: &str = "0x36";
const SLOT_VALIDATOR_VALIDATOR_TO_BLS_KEY: &str = "0x37";
const SLOT_VALIDATOR_BLS_KEY_TO_VALIDATOR: &str = "0x38";
const SLOT_VALIDATOR_GAS_TIP: &str = "0x39";
```

**Go 구현**: `systemcontracts/gov_validator.go`
**Rust 대상**: `crates/consensus/wbft/src/contracts/gov_validator.rs`

**reth 통합**:
- 상태 조회: `StateProvider` trait 사용
- 스토리지 읽기: `storage(address, slot)` 호출
- BLS 공개키 파싱: RLP 디코딩

---

### 4.2 시스템 컨트랙트 업그레이드

**설명**: 특정 블록 높이에서 시스템 컨트랙트 주소 변경

**구조**:
```rust
pub struct SystemContractUpgrade {
    pub block: U256,
    pub gov_validator: Option<Address>,
    pub native_coin_adapter: Option<Address>,
    pub gov_minter: Option<Address>,
    pub gov_master_minter: Option<Address>,
    pub gov_council: Option<Address>,
}
```

**주요 기능**:
- 블록 높이별 활성 컨트랙트 주소 결정
- `GetSystemContracts(blockNumber) -> SystemContracts`
- 체인스펙에 업그레이드 일정 정의

**Go 구현**: `consensus/wbft/config.go`, `params/config.go`
**Rust 대상**: `crates/consensus/wbft/src/config.rs`

**reth ChainSpec 통합**:
- `ChainSpec`에 `WbftConfig` 추가
- 하드포크와 유사한 블록 기반 활성화

---

### 4.3 상태 전환 훅 (State Transition Hooks)

**설명**: 시스템 컨트랙트 업그레이드시 상태 초기화

**주요 기능**:
- 업그레이드 블록에서 컨트랙트 상태 설정
- `GetSystemContractsStateTransition(config, blockNum) -> StateTransition`
- Genesis 블록에서 초기 검증자 세트 설정

**Go 구현**: `consensus/wbft/config.go:GetSystemContractsStateTransition`
**Rust 대상**: `crates/consensus/wbft/src/state_transition.rs`

**reth 통합**:
- `execute_block` 훅에서 상태 전환 적용
- Genesis 초기화 로직 확장

---

## 5. 합의 엔진 통합 (Consensus Engine Integration)

### 5.1 Consensus Trait 구현

**설명**: reth의 `Consensus` trait 구현

**필수 메서드**:
```rust
impl<B: Block> Consensus<B> for WbftConsensus {
    type Error = ConsensusError;

    fn validate_body_against_header(
        &self,
        body: &B::Body,
        header: &SealedHeader<B::Header>
    ) -> Result<(), Self::Error> {
        // 트랜잭션 루트 검증
        // Withdrawals 루트 검증 (해당되는 경우)
    }

    fn validate_block_pre_execution(&self, block: &SealedBlock<B>) -> Result<(), Self::Error> {
        // 트랜잭션 서명 복구
        // 블록 기본 검증
    }
}
```

**Go 구현**: `consensus/wbft/engine/engine.go`
**Rust 대상**: `crates/consensus/wbft/src/consensus.rs`

---

### 5.2 HeaderValidator Trait 구현

**설명**: 헤더 검증 로직

**필수 메서드**:
```rust
impl<H: BlockHeader> HeaderValidator<H> for WbftConsensus {
    fn validate_header(&self, header: &SealedHeader<H>) -> Result<(), ConsensusError> {
        // Extra data 파싱 및 검증
        // BLS 서명 검증 (prepared + committed seals)
        // Round 검증
        // Epoch 전환 검증
    }

    fn validate_header_against_parent(
        &self,
        header: &SealedHeader<H>,
        parent: &SealedHeader<H>
    ) -> Result<(), ConsensusError> {
        // 블록 번호 연속성
        // 타임스탬프 증가 검증
        // Gas limit 변경 검증
        // 제안자 검증
    }
}
```

**검증 항목**:
- Extra data 크기 및 형식
- BLS 서명 집계 검증
- Quorum 충족 확인 (bitmap 분석)
- 제안자가 올바른지 확인
- Epoch 경계에서 EpochInfo 존재 확인

**Go 구현**: `consensus/wbft/engine/engine.go:VerifyHeader`
**Rust 대상**: `crates/consensus/wbft/src/validation.rs`

---

### 5.3 FullConsensus Trait 구현

**설명**: 실행 후 검증

**필수 메서드**:
```rust
impl<N: NodePrimitives> FullConsensus<N> for WbftConsensus {
    fn validate_block_post_execution(
        &self,
        block: &RecoveredBlock<N::Block>,
        result: &BlockExecutionResult<N::Receipt>
    ) -> Result<(), ConsensusError> {
        // Gas used 검증
        // Receipts root 검증
        // Logs bloom 검증
        // 시스템 컨트랙트 상태 검증 (Epoch 전환시)
    }
}
```

**Go 구현**: `consensus/wbft/engine/engine.go:Finalize`
**Rust 대상**: `crates/consensus/wbft/src/validation.rs`

---

### 5.4 블록 씰링 (Block Sealing)

**설명**: 블록에 합의 증명 추가

**프로세스**:
1. `Prepare()` - 블록 준비, 기본 헤더 필드 설정
2. 합의 프로토콜 실행 (PRE-PREPARE → PREPARE → COMMIT)
3. `Finalize()` - 상태 업데이트, 리워드 적용
4. `CommitHeader()` - Extra data에 BLS 서명 추가
5. 블록 체인에 삽입

**Go 구현**: `consensus/wbft/engine/engine.go:Seal`, `consensus/wbft/backend/engine.go`
**Rust 대상**: `crates/consensus/wbft/src/sealer.rs`

**reth 통합**:
- `PayloadBuilder` trait 확장
- 합의 완료 대기 메커니즘 (async channel)

---

## 6. 네트워크 프로토콜 확장 (Network Protocol Extension)

### 6.1 WBFT 프로토콜 메시지

**메시지 코드**:
```rust
pub const MSG_PREPREPARE: u8 = 0x12;
pub const MSG_PREPARE: u8 = 0x13;
pub const MSG_COMMIT: u8 = 0x14;
pub const MSG_ROUNDCHANGE: u8 = 0x15;
```

**Go 구현**: `consensus/wbft/messages/message.go`
**Rust 대상**: `crates/consensus/wbft/src/protocol.rs`

---

### 6.2 P2P 핸들러

**설명**: devp2p 프로토콜 확장

**주요 기능**:
- 메시지 인코딩/디코딩 (RLP)
- 메시지 라우팅 (코드별)
- 중복 제거 (해시 기반 LRU)
- 피어 관리 (검증자 피어 우선)

**Go 구현**: `eth/handler.go`, `consensus/wbft/backend/handler.go`
**Rust 대상**: `crates/consensus/wbft/src/network/handler.rs`

**reth-network 통합**:
- `ProtocolHandler` trait 구현
- `NetworkHandle`로 메시지 전송
- `RlpxSubProtocol` 정의

---

### 6.3 검증자 피어 검색

**설명**: 검증자 노드 간 연결 우선 순위

**주요 기능**:
- 검증자 ENR 레코드 식별
- 검증자 피어 우선 연결
- 최소 검증자 연결 수 유지

**Go 구현**: `eth/discovery.go`, `p2p/server.go`
**Rust 대상**: `crates/consensus/wbft/src/network/discovery.rs`

**reth 통합**:
- `Discovery` trait 확장
- ENR에 검증자 플래그 추가

---

## 7. 설정 및 체인스펙 (Configuration & ChainSpec)

### 7.1 WBFT 설정 구조

**구조**:
```rust
pub struct WbftConfig {
    pub request_timeout: u64,           // 밀리초
    pub block_period: u64,              // 초
    pub proposer_policy: ProposerPolicy,
    pub epoch: u64,                     // 블록 수
    pub allowed_future_block_time: u64, // 초
    pub max_request_timeout_seconds: u64,
    pub transitions: Vec<Transition>,
    pub system_contract_upgrades: Vec<SystemContractUpgrade>,
}

pub struct Transition {
    pub block: U256,
    pub request_timeout_seconds: Option<u64>,
    pub block_period_seconds: Option<u64>,
    pub epoch_length: Option<u64>,
    pub proposer_policy: Option<u32>,
    pub max_request_timeout_seconds: Option<u64>,
}
```

**Go 구현**: `consensus/wbft/config.go`
**Rust 대상**: `crates/consensus/wbft/src/config.rs`

---

### 7.2 ChainSpec 통합

**설명**: reth ChainSpec에 WBFT 설정 추가

**확장 방법**:
```rust
pub struct ChainSpec {
    // 기존 필드들...
    pub wbft: Option<WbftConfig>,
}
```

**주요 기능**:
- Genesis 블록에서 초기 검증자 세트 로드
- 하드포크와 유사한 전환 관리
- `GetConfig(blockNumber) -> WbftConfig` 동적 설정

**Go 구현**: `params/config.go`
**Rust 대상**: `crates/chainspec/src/spec.rs`

---

### 7.3 Genesis 초기화

**설명**: Genesis 블록에 WBFT Extra data 설정

**주요 기능**:
- 초기 검증자 목록
- BLS 공개키 목록
- 초기 gas tip
- Epoch info 생성

**Go 구현**: `consensus/wbft/config.go:CreateInitialExtraData`
**Rust 대상**: `crates/consensus/wbft/src/genesis.rs`

**Genesis JSON 형식**:
```json
{
  "config": {
    "wbft": {
      "validators": ["0x...", "0x..."],
      "blsPublicKeys": ["0x...", "0x..."],
      "epoch": 10,
      "blockPeriod": 1
    }
  }
}
```

---

## 8. 테스팅 및 시뮬레이션 (Testing & Simulation)

### 8.1 단위 테스트

**테스트 영역**:
- BLS 서명 생성/검증/집계
- 메시지 인코딩/디코딩
- ValidatorSet 로직
- 제안자 선출 알고리즘
- Quorum 계산
- Extra data 파싱

**Go 구현**: `consensus/wbft/*_test.go`
**Rust 대상**: `crates/consensus/wbft/src/*/tests.rs`

---

### 8.2 통합 테스트

**테스트 시나리오**:
- 단일 검증자 블록 생성
- 3/4/7 검증자 합의
- 라운드 변경 시나리오
- 네트워크 파티션 복구
- Epoch 전환
- 시스템 컨트랙트 업그레이드

**Go 구현**: `consensus/wbft/core/*_test.go`
**Rust 대상**: `crates/consensus/wbft/tests/*.rs`

---

### 8.3 벤치마크

**측정 항목**:
- BLS 서명 성능
- 메시지 처리 처리량
- 합의 지연시간
- 메모리 사용량

**Rust 대상**: `crates/consensus/wbft/benches/*.rs`

---

## 9. 모니터링 및 관찰성 (Monitoring & Observability)

### 9.1 메트릭

**주요 메트릭**:
- `wbft_round_duration` - 라운드 소요 시간
- `wbft_round_changes_total` - 라운드 변경 횟수
- `wbft_messages_received_total{type}` - 메시지 수신 카운터
- `wbft_consensus_state` - 현재 합의 상태
- `wbft_validator_set_size` - 검증자 수
- `wbft_proposer_index` - 현재 제안자 인덱스

**Go 구현**: `consensus/wbft/backend/backend.go` (metrics 사용)
**Rust 대상**: `crates/consensus/wbft/src/metrics.rs`

**reth 통합**: `metrics` 크레이트 사용

---

### 9.2 로깅

**로그 레벨**:
- **TRACE**: 메시지 상세 내용
- **DEBUG**: 상태 전이, 타이머 이벤트
- **INFO**: 라운드 변경, 블록 완료
- **WARN**: 타임아웃, 잘못된 메시지
- **ERROR**: 합의 실패, 검증 오류

**Go 구현**: `log` 패키지 사용
**Rust 대상**: `tracing` 크레이트 사용

---

### 9.3 디버깅 API

**RPC 메서드**:
- `wbft_getValidators(blockNumber)` - 검증자 목록 조회
- `wbft_getSnapshot(blockNumber)` - 합의 스냅샷
- `wbft_getCurrentRound()` - 현재 라운드 정보
- `wbft_getMessageStats()` - 메시지 통계

**Go 구현**: `consensus/wbft/backend/api.go`
**Rust 대상**: `crates/consensus/wbft/src/rpc.rs`

**reth-rpc 통합**: 새로운 네임스페이스 추가

---

## 10. 문서화 (Documentation)

### 10.1 아키텍처 문서

**내용**:
- WBFT 프로토콜 개요
- 상태 머신 다이어그램
- 메시지 플로우 차트
- 보안 모델 설명

**위치**: `docs/consensus/wbft/architecture.md`

---

### 10.2 통합 가이드

**내용**:
- reth에 WBFT 추가 방법
- 설정 파일 예제
- Genesis 파일 생성
- 검증자 노드 설정

**위치**: `docs/consensus/wbft/integration.md`

---

### 10.3 API 레퍼런스

**내용**:
- RPC 메서드 문서
- 메시지 형식 정의
- 설정 옵션 설명

**위치**: `docs/consensus/wbft/api.md`

---

## 구현 복잡도 평가

| 기능 영역 | 복잡도 | 예상 공수 | 우선순위 |
|---------|-------|---------|---------|
| BLS 서명 및 집계 | 중 | 1-2주 | 높음 |
| 메시지 타입 및 인코딩 | 낮 | 1주 | 높음 |
| 상태 머신 | 높음 | 2-3주 | 높음 |
| 검증자 관리 | 중 | 1-2주 | 높음 |
| 네트워크 프로토콜 | 높음 | 2-3주 | 중간 |
| 헤더 검증 | 중 | 1주 | 높음 |
| 블록 씰링 | 높음 | 2주 | 중간 |
| 시스템 컨트랙트 통합 | 중 | 1-2주 | 중간 |
| ChainSpec 통합 | 낮 | 3-5일 | 높음 |
| 테스팅 | 중 | 지속적 | 높음 |
| 문서화 | 낮 | 1주 | 낮음 |

**총 예상 공수**: 약 3-4개월 (1명 기준)

---

## 의존성 및 외부 크레이트

**필수 크레이트**:
- `blst` - BLS12-381 서명
- `tokio` - 비동기 런타임
- `alloy-rlp` - RLP 인코딩/디코딩
- `reth-primitives` - 블록/헤더 타입
- `reth-consensus` - Consensus 트레잇
- `reth-network` - P2P 네트워킹
- `reth-chainspec` - 체인 설정
- `metrics` - 메트릭 수집
- `tracing` - 로깅

**선택적 크레이트**:
- `proptest` - 속성 기반 테스트
- `criterion` - 벤치마킹

---

## 마이그레이션 전략

1. **Phase 1 - 기초 구조** (2-3주)
   - BLS 서명 구현
   - 메시지 타입 정의
   - 기본 데이터 구조

2. **Phase 2 - 합의 프로토콜** (3-4주)
   - 상태 머신 구현
   - 메시지 핸들러
   - 타이머 및 라운드 관리

3. **Phase 3 - 검증 및 통합** (3-4주)
   - Consensus trait 구현
   - 헤더 검증
   - ChainSpec 통합

4. **Phase 4 - 네트워크 및 고급 기능** (3-4주)
   - P2P 프로토콜
   - 시스템 컨트랙트
   - 블록 씰링

5. **Phase 5 - 테스트 및 최적화** (2-3주)
   - 통합 테스트
   - 성능 최적화
   - 문서화

---

## 리스크 및 고려사항

**기술적 리스크**:
1. **Go → Rust 패러다임 전환**: 채널 기반 → async/await
2. **BLS 라이브러리 호환성**: blst vs go-ethereum/crypto/bls
3. **P2P 프로토콜 확장**: reth-network 커스터마이징
4. **성능 최적화**: Rust의 zero-cost abstractions 활용

**통합 리스크**:
1. **reth 아키텍처 변경**: Upstream 변경 추적 필요
2. **ChainSpec 확장성**: 기존 체인과의 호환성
3. **Engine API 통합**: PoS Engine API와의 관계

**완화 전략**:
1. 단계별 구현 및 테스트
2. 기존 Optimism 합의 참조
3. 포괄적인 단위/통합 테스트
4. 지속적인 코드 리뷰

---

## 성공 기준

**기능 완성도**:
- [ ] 모든 메시지 타입 구현 및 테스트
- [ ] BLS 서명 집계 정확성 100%
- [ ] Quorum 계산 정확성 검증
- [ ] 라운드 변경 시나리오 통과
- [ ] Epoch 전환 정상 작동

**성능**:
- [ ] 블록 타임 1초 달성 (4 검증자)
- [ ] 라운드 변경 < 5초 (타임아웃)
- [ ] BLS 서명 검증 < 10ms
- [ ] 메시지 처리 처리량 > 1000 msg/s

**통합**:
- [ ] reth 메인넷 동기화 가능
- [ ] 기존 Ethereum 블록 검증 가능
- [ ] WBFT 블록 생성 및 검증
- [ ] P2P 네트워크 정상 작동

**품질**:
- [ ] 테스트 커버리지 > 80%
- [ ] 모든 public API 문서화
- [ ] 통합 가이드 완성
- [ ] 벤치마크 결과 기록
