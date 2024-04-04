
use candid::{CandidType, Decode, Deserialize, Encode};
use ic_cdk::{caller, query, update};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    {BoundedStorable, DefaultMemoryImpl,StableBTreeMap, Storable, Cell},
};
use serde::Serialize;
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;
const MAX_VALUE_SIZE: u32 = 5000;

#[derive(CandidType, Deserialize, Serialize, Clone)]
enum Choice {
    Approve,
    Reject,
    Pass,
}

#[derive(CandidType, Deserialize, Serialize)]
enum ShowError {
    AlreadyAdded,
    ProductIsNotActive,
    NoSuchProduct,
    AccessRejected,
    UpdateError(String), // Improved error message
}

#[derive(CandidType, Deserialize, Clone, Serialize)]
struct Product {
    id: u64,
    description: String,
    approve: u32,
    reject: u32,
    pass: u32,
    is_active: bool,
    voted: Vec<candid::Principal>,
    owner: candid::Principal,
}

#[derive(CandidType, Deserialize, Clone, Serialize)]
struct CreateProduct {
    description: String,
    is_active: bool,
}

impl Storable for Product {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Product {
    const MAX_SIZE: u32 = MAX_VALUE_SIZE;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static PRODUCT_MAP: RefCell<StableBTreeMap<u64, Product, Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))))
    );
}

#[query]
fn get_product(id: u64) -> Option<Product> {
    PRODUCT_MAP.with(|p| p.borrow().get(&id))
}

#[query]
fn get_product_count() -> u64 {
    PRODUCT_MAP.with(|p| p.borrow().len())
}

#[update]
fn create_product(product: CreateProduct) -> Option<Product> {
    let id = ID_COUNTER
    .with(|counter| {
        let current_value = *counter.borrow().get();
        counter.borrow_mut().set(current_value + 1)
    })
    .expect("cannot increment id counter");
    let value: Product = Product {
        id,
        description: product.description,
        approve: 0u32,
        reject: 0u32,
        pass: 0u32,
        is_active: product.is_active,
        voted: vec![],
        owner: caller(),
    };

    PRODUCT_MAP.with(|p| p.borrow_mut().insert(id, value));
    Some(PRODUCT_MAP.with(|p| p.borrow().get(&id).unwrap()))
}

#[update]
fn edit_product(id: u64, product: CreateProduct) -> Result<(), ShowError> {
    let result = PRODUCT_MAP.with(|p| {
        let old_product_opt: Option<Product> = p.borrow().get(&id);
        let old_product = old_product_opt.ok_or(ShowError::NoSuchProduct)?;
        if caller() != old_product.owner {
            return Err(ShowError::AccessRejected);
        };

        let value: Product = Product {
            description: product.description,
            is_active: product.is_active,
            ..old_product
        };

        p.borrow_mut().insert(value.id, value).ok_or(ShowError::UpdateError("Insert failed".to_string()))
    });
    if result.is_ok() {
        Ok(())
    }else {
        return Err(result.err().unwrap())
    }
}

#[update]
fn end_product(id: u64) -> Result<(), ShowError> {
    let result = PRODUCT_MAP.with(|p| {
        let Product_opt: Option<Product> = p.borrow().get(&id);
        let mut Product = Product_opt.ok_or(ShowError::NoSuchProduct)?;

        if caller() != Product.owner {
            return Err(ShowError::AccessRejected);
        };

        Product.is_active = false;

        p.borrow_mut().insert(id, Product).ok_or(ShowError::UpdateError("Insert failed".to_string()))
    });

    if result.is_ok() {
        Ok(())
    }else {
        return Err(result.err().unwrap())
    }

}

#[update]
fn vote(id: u64, choice: Choice) -> Result<(), ShowError> {
    let result = PRODUCT_MAP.with(|p| {
        let Product_opt: Option<Product> = p.borrow().get(&id);
        let mut Product = Product_opt.ok_or(ShowError::NoSuchProduct)?;

        let caller = caller();

        if Product.voted.contains(&caller) {
            return Err(ShowError::AlreadyAdded);
        } else if !Product.is_active {
            return Err(ShowError::ProductIsNotActive);
        };

        match choice {
            Choice::Approve => Product.approve += 1,
            Choice::Reject => Product.reject += 1,
            Choice::Pass => Product.pass += 1,
        };

        Product.voted.push(caller);

        p.borrow_mut().insert(id, Product).ok_or(ShowError::UpdateError("Insert failed".to_string()))
    });
    if result.is_ok() {
        Ok(())
    }else {
        return Err(result.err().unwrap())
    }
}

// need this to generate candid
ic_cdk::export_candid!();