# WBFT Genesis 설정 통합 가이드

## 개요

이 문서는 go-stablenet의 Anzeon/WBFT Genesis 설정을 reth로 포팅하는 상세 가이드입니다.

**참조**:
- go-stablenet: `core/genesis.go`, `params/config_wbft.go`
- reth: `crates/chainspec/`, `crates/consensus/wbft/`

---

## 1. go-stablenet Genesis 구조 분석

### 1.1 ChainConfig 구조

**파일**: `params/config.go`

```go
type ChainConfig struct {
    ChainID *big.Int `json:"chainId"`

    // 기본 하드포크
    HomesteadBlock      *big.Int `json:"homesteadBlock,omitempty"`
    EIP150Block         *big.Int `json:"eip150Block,omitempty"`
    // ... 다른 하드포크들

    // 합의 엔진 설정
    Ethash  *EthashConfig  `json:"ethash,omitempty"`
    Clique  *CliqueConfig  `json:"clique,omitempty"`
    Anzeon  *AnzeonConfig  `json:"anzeon,omitempty"`  // ← WBFT 설정

    Transitions []Transition `json:"transitions,omitempty"`
}
```

### 1.2 AnzeonConfig 구조

**파일**: `params/config_wbft.go`

```go
type AnzeonConfig struct {
    WBFT            *WBFTConfig      `json:"wbft"`
    Init            *WBFTInit        `json:"init"`
    SystemContracts *SystemContracts `json:"systemContracts"`
}

type WBFTInit struct {
    Validators    []common.Address `json:"validators"`     // 초기 검증자 주소 목록
    BLSPublicKeys []string         `json:"blsPublicKeys"`  // BLS 공개키 목록 (순서 중요!)
}

type WBFTConfig struct {
    RequestTimeoutSeconds    uint64   `json:"requestTimeoutSeconds"`
    BlockPeriodSeconds       uint64   `json:"blockPeriodSeconds"`
    ProposerPolicy           *uint64  `json:"proposerPolicy"`
    EpochLength              uint64   `json:"epochLength"`
    MaxRequestTimeoutSeconds *uint64  `json:"maxRequestTimeoutSeconds,omitempty"`
}

type SystemContracts struct {
    GovValidator      *SystemContract `json:"govValidator"`
    NativeCoinAdapter *SystemContract `json:"nativeCoinAdapter,omitempty"`
    GovMinter         *SystemContract `json:"govMinter,omitempty"`
    GovMasterMinter   *SystemContract `json:"govMasterMinter,omitempty"`
    GovCouncil        *SystemContract `json:"govCouncil,omitempty"`
}

type SystemContract struct {
    Address common.Address    `json:"address"`
    Version string            `json:"version,omitempty"`
    Params  map[string]string `json:"params,omitempty"`
}
```

### 1.3 Genesis Extra Data 생성

**파일**: `core/genesis.go:ToBlock()`

```go
func (g *Genesis) ToBlock() *types.Block {
    // Anzeon(WBFT) 활성화시
    if g.Config.AnzeonEnabled() {
        // 1. 설정 검증
        if err := g.Config.Anzeon.CheckValidity(); err != nil {
            panic(err)
        }

        // 2. WBFT Extra data 생성
        g.ExtraData, err = wbft.CreateInitialExtraData(g.Config.Anzeon)
        // -> 이 함수가 WBFTExtra를 RLP 인코딩하여 반환

        // 3. 시스템 컨트랙트 주입
        err = InjectContracts(g, g.Config)
    }

    // 블록 헤더 생성
    head := &types.Header{
        Number:     g.Number,
        Nonce:      types.EncodeNonce(g.Nonce),
        Time:       g.Timestamp,
        ParentHash: g.ParentHash,
        Extra:      g.ExtraData,  // ← WBFT extra data 포함
        GasLimit:   g.GasLimit,
        GasUsed:    g.GasUsed,
        Difficulty: g.Difficulty,
        MixDigest:  g.Mixhash,
        Coinbase:   g.Coinbase,
        BaseFee:    g.BaseFee,
        // ...
    }

    return types.NewBlock(head, txs, uncles, receipts, trie.NewStackTrie(nil))
}
```

---

## 2. reth ChainSpec 구조

### 2.1 현재 reth ChainSpec

**파일**: `crates/chainspec/src/spec.rs`

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainSpec {
    pub chain: Chain,

    pub genesis: Genesis,

    #[serde(flatten)]
    pub hardforks: ChainHardforks,

    pub genesis_hash: Option<B256>,

    pub paris_block_and_final_difficulty: Option<(u64, U256)>,

    pub deposit_contract: Option<DepositContract>,

    // 현재는 없음! 추가 필요
    // pub wbft: Option<WbftConfig>,
}
```

### 2.2 Genesis 구조

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Genesis {
    pub nonce: u64,

    pub timestamp: u64,

    pub extra_data: Bytes,  // ← 여기에 WBFT extra data 저장

    pub gas_limit: u64,

    pub difficulty: U256,

    pub mix_hash: B256,

    pub coinbase: Address,

    pub alloc: GenesisAlloc,

    // EIP-1559
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_fee_per_gas: Option<u64>,

    // ... 기타 필드
}
```

---

## 3. reth에 WBFT Genesis 통합

### 3.1 ChainSpec에 WbftConfig 추가

**파일**: `crates/chainspec/src/spec.rs`

```rust
use crate::wbft::WbftChainConfig;  // 새로 추가할 모듈

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainSpec {
    // ... 기존 필드들

    /// WBFT consensus configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wbft: Option<WbftChainConfig>,
}

impl ChainSpec {
    /// Returns true if WBFT consensus is enabled
    pub fn is_wbft(&self) -> bool {
        self.wbft.is_some()
    }

    /// Get WBFT config for specific block number
    pub fn wbft_config_at(&self, block_number: u64) -> Option<WbftConfig> {
        self.wbft.as_ref().map(|w| w.get_config(block_number))
    }
}
```

### 3.2 WbftChainConfig 정의

**파일**: `crates/chainspec/src/wbft.rs`

```rust
use alloy_primitives::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};

/// WBFT chain-level configuration (equivalent to go-stablenet's AnzeonConfig)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WbftChainConfig {
    /// WBFT consensus parameters
    pub wbft: WbftConfig,

    /// Initial validator set configuration
    pub init: WbftInit,

    /// System contracts configuration
    pub system_contracts: SystemContracts,
}

/// WBFT consensus configuration (equivalent to go-stablenet's WBFTConfig)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WbftConfig {
    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout_seconds: u64,

    /// Block period in seconds
    #[serde(default = "default_block_period")]
    pub block_period_seconds: u64,

    /// Proposer selection policy (0 = RoundRobin, 1 = Sticky)
    #[serde(default)]
    pub proposer_policy: u64,

    /// Epoch length in blocks
    #[serde(default = "default_epoch_length")]
    pub epoch_length: u64,

    /// Maximum request timeout in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_request_timeout_seconds: Option<u64>,
}

fn default_request_timeout() -> u64 { 2 }
fn default_block_period() -> u64 { 1 }
fn default_epoch_length() -> u64 { 10 }

/// Initial WBFT validator set
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WbftInit {
    /// Initial validator addresses (order matters!)
    pub validators: Vec<Address>,

    /// BLS public keys in hex format (must match validators order)
    #[serde(rename = "blsPublicKeys")]
    pub bls_public_keys: Vec<String>,  // "0x..." 형식
}

/// System contracts configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemContracts {
    /// GovValidator contract
    pub gov_validator: SystemContract,

    /// Optional contracts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_coin_adapter: Option<SystemContract>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_minter: Option<SystemContract>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_master_minter: Option<SystemContract>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_council: Option<SystemContract>,
}

/// Individual system contract configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemContract {
    /// Contract address
    pub address: Address,

    /// Contract version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Additional parameters (e.g., gasTip)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub params: std::collections::HashMap<String, String>,
}

/// Block-based WBFT configuration transitions
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WbftTransition {
    /// Block number where transition occurs
    pub block: U256,

    /// New request timeout (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_timeout_seconds: Option<u64>,

    /// New block period (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_period_seconds: Option<u64>,

    /// New epoch length (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch_length: Option<u64>,

    /// New proposer policy (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposer_policy: Option<u64>,

    /// New max timeout (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_request_timeout_seconds: Option<u64>,
}

/// System contract upgrade specification
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemContractUpgrade {
    /// Block number where upgrade occurs
    pub block: U256,

    /// New contract addresses (only specified ones are updated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_validator: Option<Address>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_coin_adapter: Option<Address>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_minter: Option<Address>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_master_minter: Option<Address>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gov_council: Option<Address>,
}

impl WbftChainConfig {
    /// Get active WBFT config for given block number
    pub fn get_config(&self, _block_number: u64) -> WbftConfig {
        // TODO: Apply transitions based on block number
        self.wbft.clone()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        // 1. 검증자와 BLS 키 개수 일치 확인
        if self.init.validators.len() != self.init.bls_public_keys.len() {
            return Err(format!(
                "Validator count ({}) doesn't match BLS key count ({})",
                self.init.validators.len(),
                self.init.bls_public_keys.len()
            ));
        }

        // 2. 검증자 최소 개수 확인 (최소 1개)
        if self.init.validators.is_empty() {
            return Err("At least one validator required".to_string());
        }

        // 3. BLS 공개키 형식 검증
        for (i, key) in self.init.bls_public_keys.iter().enumerate() {
            if !key.starts_with("0x") {
                return Err(format!("BLS key {} must start with 0x", i));
            }

            // BLS 공개키는 48 bytes = 96 hex chars + "0x"
            if key.len() != 98 {
                return Err(format!(
                    "BLS key {} has invalid length {} (expected 98)",
                    i, key.len()
                ));
            }
        }

        // 4. Epoch 길이 최소값 확인
        if self.wbft.epoch_length == 0 {
            return Err("Epoch length must be > 0".to_string());
        }

        Ok(())
    }

    /// Get initial BLS public keys as bytes
    pub fn initial_bls_public_keys(&self) -> Result<Vec<Vec<u8>>, String> {
        self.init.bls_public_keys
            .iter()
            .map(|s| {
                hex::decode(s.trim_start_matches("0x"))
                    .map_err(|e| format!("Invalid BLS key hex: {}", e))
            })
            .collect()
    }
}
```

### 3.3 Genesis Extra Data 생성 함수

**파일**: `crates/consensus/wbft/src/genesis.rs`

```rust
use alloy_primitives::{Address, Bytes, U256};
use alloy_rlp::Encodable;
use crate::{WbftExtra, EpochInfo, Candidate};

/// Create initial WBFT extra data for genesis block
pub fn create_initial_extra_data(
    config: &WbftChainConfig
) -> Result<Bytes, String> {
    // 1. 설정 검증
    config.validate()?;

    // 2. EpochInfo 생성
    let epoch_info = create_initial_epoch_info(config)?;

    // 3. Initial gas tip 파싱
    let gas_tip = config.system_contracts.gov_validator.params
        .get("gasTip")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1_000_000_000); // 기본값: 1 Gwei

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

    // 5. RLP 인코딩
    let mut encoded = Vec::new();
    extra.encode(&mut encoded);

    Ok(Bytes::from(encoded))
}

/// Create initial epoch info from genesis config
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
            diligence: 950_000, // Default: 95% (단위: 10^-6)
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

---

## 4. Genesis JSON 형식

### 4.1 WBFT Genesis JSON 예제

```json
{
  "config": {
    "chainId": 1234,
    "homesteadBlock": 0,
    "eip150Block": 0,
    "eip155Block": 0,
    "eip158Block": 0,
    "byzantiumBlock": 0,
    "constantinopleBlock": 0,
    "petersburgBlock": 0,
    "istanbulBlock": 0,
    "berlinBlock": 0,
    "londonBlock": 0,

    "wbft": {
      "wbft": {
        "requestTimeoutSeconds": 2,
        "blockPeriodSeconds": 1,
        "proposerPolicy": 0,
        "epochLength": 10,
        "maxRequestTimeoutSeconds": 10
      },
      "init": {
        "validators": [
          "0x1234567890123456789012345678901234567890",
          "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
          "0x9876543210987654321098765432109876543210"
        ],
        "blsPublicKeys": [
          "0xaec493af8fa358a1c6f05499f2dd712721ade88c477d21b799d38e9b84582b6fbe4f4adc21e1e454bc37522eb3478b9b",
          "0xb1ae18fdcbcc6a80d7a0c4cfec1a04bc1bee78e519eaadd689108077d946e0849a2c30ac96462be32023f34ca67ebcf6",
          "0x8f7d9e2c4b1a6f3e5d8c9b7a4e2f1d0c9b8a7e6d5c4b3a2f1e0d9c8b7a6f5e4d3c2b1a0f9e8d7c6b5a4f3e2d1c0b9a8"
        ]
      },
      "systemContracts": {
        "govValidator": {
          "address": "0x0000000000000000000000000000000000000400",
          "version": "v1",
          "params": {
            "gasTip": "1000000000"
          }
        },
        "govCouncil": {
          "address": "0x0000000000000000000000000000000000000401",
          "version": "v1"
        }
      }
    }
  },
  "nonce": "0x0",
  "timestamp": "0x0",
  "extraData": "0x",
  "gasLimit": "0xe4e1c0",
  "difficulty": "0x1",
  "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "coinbase": "0x0000000000000000000000000000000000000000",
  "alloc": {
    "0x1234567890123456789012345678901234567890": {
      "balance": "0x200000000000000000000000000000000000000000000000000000000000000"
    }
  }
}
```

### 4.2 Transitions 예제

시간에 따른 설정 변경:

```json
{
  "config": {
    "wbft": {
      "wbft": {
        "requestTimeoutSeconds": 2,
        "blockPeriodSeconds": 1,
        "proposerPolicy": 0,
        "epochLength": 10
      },
      "init": { "..." },
      "systemContracts": { "..." }
    },

    "transitions": [
      {
        "block": "100",
        "requestTimeoutSeconds": 3,
        "epochLength": 20
      },
      {
        "block": "1000",
        "proposerPolicy": 1,
        "maxRequestTimeoutSeconds": 15
      }
    ]
  }
}
```

---

## 5. Genesis 초기화 플로우

### 5.1 ChainSpec 로드

**파일**: `crates/chainspec/src/spec.rs`

```rust
impl ChainSpec {
    /// Load chain spec from JSON file
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut spec: ChainSpec = serde_json::from_str(json)?;

        // WBFT 설정 검증
        if let Some(ref wbft) = spec.wbft {
            wbft.validate()
                .map_err(|e| serde_json::Error::custom(e))?;
        }

        Ok(spec)
    }
}
```

### 5.2 Genesis 블록 생성

**파일**: `crates/chainspec/src/genesis.rs`

```rust
use crate::wbft::create_initial_extra_data;

impl Genesis {
    /// Convert genesis spec to block with WBFT support
    pub fn to_block(
        &self,
        wbft_config: Option<&WbftChainConfig>
    ) -> Result<SealedBlock, String> {
        // 1. Extra data 설정
        let extra_data = if let Some(config) = wbft_config {
            // WBFT extra data 생성
            create_initial_extra_data(config)?
        } else {
            // 기본 extra data
            self.extra_data.clone()
        };

        // 2. 헤더 생성
        let header = Header {
            parent_hash: B256::ZERO,
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            beneficiary: self.coinbase,
            state_root: self.calculate_state_root()?,
            transactions_root: EMPTY_TRANSACTIONS,
            receipts_root: EMPTY_RECEIPTS,
            logs_bloom: Bloom::default(),
            difficulty: self.difficulty,
            number: 0,
            gas_limit: self.gas_limit,
            gas_used: 0,
            timestamp: self.timestamp,
            mix_hash: self.mix_hash,
            nonce: self.nonce,
            base_fee_per_gas: self.base_fee_per_gas,
            withdrawals_root: None,
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root: None,
            requests_hash: None,
            extra_data,  // ← WBFT extra data 포함
        };

        Ok(SealedBlock::new(header, BlockBody::default()))
    }
}
```

### 5.3 Node 초기화에서 사용

**파일**: `crates/node/src/builder.rs` (가상 예제)

```rust
impl NodeBuilder {
    pub fn with_genesis(mut self, genesis_path: &Path) -> Result<Self> {
        // 1. Genesis JSON 로드
        let genesis_json = std::fs::read_to_string(genesis_path)?;
        let chain_spec = ChainSpec::from_json(&genesis_json)?;

        // 2. WBFT Genesis 블록 생성
        let genesis_block = if let Some(ref wbft) = chain_spec.wbft {
            chain_spec.genesis.to_block(Some(wbft))?
        } else {
            chain_spec.genesis.to_block(None)?
        };

        // 3. 데이터베이스에 Genesis 커밋
        self.commit_genesis(genesis_block)?;

        Ok(self)
    }
}
```

---

## 6. 구현 체크리스트

### Phase 1: ChainSpec 확장 (1주)

- [ ] **작업 6.1.1**: WbftChainConfig 구조체 정의
  - 파일: `crates/chainspec/src/wbft.rs`
  - 구현: WbftChainConfig, WbftConfig, WbftInit, SystemContracts
  - 테스트: JSON 직렬화/역직렬화
  - 예상 시간: 2일

- [ ] **작업 6.1.2**: ChainSpec에 wbft 필드 추가
  - 파일: `crates/chainspec/src/spec.rs`
  - 수정: ChainSpec 구조체에 `pub wbft: Option<WbftChainConfig>` 추가
  - 테스트: 기존 ChainSpec 역호환성 확인
  - 예상 시간: 1일

- [ ] **작업 6.1.3**: 설정 검증 로직
  - 파일: `crates/chainspec/src/wbft.rs`
  - 구현: `WbftChainConfig::validate()` 메서드
  - 검증 항목:
    - 검증자 수 = BLS 키 수
    - BLS 키 형식 검증 (0x + 96 hex chars)
    - Epoch 길이 > 0
    - 시스템 컨트랙트 주소 유효성
  - 테스트: 유효/무효 설정 케이스
  - 예상 시간: 2일

### Phase 2: Genesis Extra Data 생성 (1주)

- [ ] **작업 6.2.1**: create_initial_extra_data 함수
  - 파일: `crates/consensus/wbft/src/genesis.rs`
  - 구현: WbftChainConfig → Bytes 변환
  - 단계:
    1. 설정 검증
    2. EpochInfo 생성
    3. Gas tip 파싱
    4. WbftExtra 구조체 생성
    5. RLP 인코딩
  - 예상 시간: 2일

- [ ] **작업 6.2.2**: create_initial_epoch_info 함수
  - 파일: `crates/consensus/wbft/src/genesis.rs`
  - 구현: 초기 검증자 → EpochInfo 변환
  - 단계:
    1. Candidates 생성 (diligence = 95%)
    2. Validators 인덱스 생성
    3. BLS 공개키 hex → bytes 변환
  - 예상 시간: 1일

- [ ] **작업 6.2.3**: Genesis::to_block 통합
  - 파일: `crates/chainspec/src/genesis.rs`
  - 수정: to_block 메서드에 WBFT 지원 추가
  - 로직:
    ```rust
    let extra_data = if let Some(wbft) = wbft_config {
        create_initial_extra_data(wbft)?
    } else {
        self.extra_data.clone()
    };
    ```
  - 예상 시간: 2일

### Phase 3: JSON 파싱 및 검증 (3일)

- [ ] **작업 6.3.1**: Genesis JSON 예제 생성
  - 파일: `testdata/wbft-genesis.json`
  - 내용: 완전한 WBFT Genesis 설정
  - 3개 검증자, 시스템 컨트랙트 포함
  - 예상 시간: 0.5일

- [ ] **작업 6.3.2**: JSON 파싱 테스트
  - 파일: `crates/chainspec/src/wbft/tests.rs`
  - 테스트 케이스:
    - `test_parse_wbft_genesis_json` - 정상 파싱
    - `test_invalid_bls_key_count` - 검증자/키 불일치
    - `test_invalid_bls_key_format` - 잘못된 hex
    - `test_missing_required_fields` - 필수 필드 누락
  - 예상 시간: 1일

- [ ] **작업 6.3.3**: Genesis 블록 생성 테스트
  - 파일: `crates/consensus/wbft/tests/genesis.rs`
  - 테스트 시나리오:
    - Genesis 블록 생성
    - Extra data 파싱
    - EpochInfo 검증
    - Gas tip 검증
  - 예상 시간: 1일

### Phase 4: Node 통합 (1주)

- [ ] **작업 6.4.1**: CLI에서 Genesis 로드
  - 파일: `crates/node/src/cli.rs` (예시)
  - 구현: `--genesis <path>` 플래그 처리
  - 예상 시간: 1일

- [ ] **작업 6.4.2**: Database에 Genesis 커밋
  - 파일: `crates/node/src/genesis.rs` (예시)
  - 구현: WBFT Genesis 블록을 DB에 저장
  - 검증: Genesis hash 일치 확인
  - 예상 시간: 2일

- [ ] **작업 6.4.3**: ChainSpec 캐싱
  - 구현: ChainSpec을 Arc로 공유
  - 합의 엔진에서 wbft 설정 접근
  - 예상 시간: 1일

- [ ] **작업 6.4.4**: 통합 테스트
  - 테스트: 전체 플로우 검증
    1. Genesis JSON 로드
    2. Genesis 블록 생성
    3. DB 저장
    4. 블록 검증
  - 예상 시간: 1일

---

## 7. go-stablenet vs reth 매핑

| go-stablenet | reth | 위치 |
|--------------|------|------|
| `params.ChainConfig.Anzeon` | `ChainSpec.wbft` | `crates/chainspec/src/spec.rs` |
| `params.AnzeonConfig` | `WbftChainConfig` | `crates/chainspec/src/wbft.rs` |
| `params.WBFTConfig` | `WbftConfig` | `crates/chainspec/src/wbft.rs` |
| `params.WBFTInit` | `WbftInit` | `crates/chainspec/src/wbft.rs` |
| `wbft.CreateInitialExtraData()` | `create_initial_extra_data()` | `crates/consensus/wbft/src/genesis.rs` |
| `wbft.CreateInitialEpochInfo()` | `create_initial_epoch_info()` | `crates/consensus/wbft/src/genesis.rs` |
| `core.Genesis.ToBlock()` | `Genesis::to_block()` | `crates/chainspec/src/genesis.rs` |

---

## 8. 주의사항

### 8.1 BLS 공개키 형식

- **go-stablenet**: hex string "0x..." (98 chars)
- **reth**: 동일 형식 유지, 내부적으로 `Vec<u8>` 변환

### 8.2 RLP 인코딩 호환성

- WbftExtra RLP 인코딩이 go-stablenet과 **정확히 일치**해야 함
- 필드 순서, 타입 동일하게 유지
- 테스트: go-stablenet Genesis와 동일한 extra data 생성 확인

### 8.3 ChainSpec 역호환성

- 기존 Ethereum ChainSpec 파싱에 영향 없어야 함
- `wbft` 필드는 `Option<>`으로 선택적
- 기존 테스트 모두 통과 필요

---

## 9. 검증 방법

### 9.1 Genesis 블록 해시 비교

```bash
# go-stablenet에서 Genesis 생성
gstable init genesis.json

# reth에서 Genesis 생성
reth init --genesis genesis.json

# 블록 해시 비교 (동일해야 함)
gstable db inspect 0
reth db inspect 0
```

### 9.2 Extra Data 검증

```rust
#[test]
fn test_extra_data_compatibility() {
    let config = WbftChainConfig { /* ... */ };
    let extra = create_initial_extra_data(&config).unwrap();

    // go-stablenet에서 생성한 extra data와 비교
    let expected = hex::decode("...").unwrap();
    assert_eq!(extra.as_ref(), &expected);
}
```

---

## 10. 문서 및 예제

### 10.1 사용자 가이드

작성 필요: `docs/wbft/genesis-setup.md`

내용:
- WBFT Genesis JSON 작성 방법
- 검증자 설정 가이드
- BLS 키 생성 방법
- 시스템 컨트랙트 설정

### 10.2 개발자 가이드

작성 필요: `docs/wbft/genesis-dev.md`

내용:
- ChainSpec 확장 방법
- Genesis 초기화 플로우
- 테스트 케이스 작성

---

**작성일**: 2025-XX-XX
**다음 업데이트**: Phase 1 완료 후
