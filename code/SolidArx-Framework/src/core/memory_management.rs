//! Modulo per la gestione della memoria in base al tipo di applicazione.
//!
//! Questo modulo fornisce un gestore della memoria che implementa diverse strategie di allocazione
//! a seconda del tipo di applicazione. Le strategie attualmente supportate includono:
//! - `Standard`: allocazione standard, utilizza il sistema di allocazione predefinito di Rust.
//! - `PoolBased`: allocazione basata su un pool di buffer pre-allocati per migliorare le prestazioni.
//! - `CustomEmbedded`: allocazione personalizzata per applicazioni embedded con requisiti specifici.
//!
//! È possibile configurare la dimensione dei buffer e del pool utilizzando la struttura `MemoryConfig`.

use crate::config::{
    global_config::ApplicationType,
    memory_config::MemoryConfig,
};
use crate::core::system_core::CoreError;
use log::{info};
use std::collections::VecDeque;
use std::io::{self, Write};

// Importing di lazy static per la creazione di strutture dati globali


// struttura globale TASKS_IN_MEMORY che mantiene tutti i Task in memoria
#[cfg(feature = "crud")]
use std::sync::Mutex;
#[cfg(feature = "crud")]
use lazy_static::lazy_static;
#[cfg(feature = "crud")]
use std::collections::HashMap;
#[cfg(feature = "crud")]
use crate::crud::models::default::{
    task::model::Task,
    device::model::Device,
    job::model::Job,
    macro_script::model::Macro,
    sensor_data::model::SensorData,
    log_event::model::LogEvent,
    command::model::Command,
    configuration::model::Configuration,
};
#[cfg(feature = "crud")]
lazy_static! {
    pub static ref TASKS_IN_MEMORY: Mutex<HashMap<u32, Task>> = Mutex::new(HashMap::new());
    pub static ref CONFIGURATIONS_IN_MEMORY: Mutex<HashMap<u32, Configuration>> = Mutex::new(HashMap::new());
    pub static ref DEVICES_IN_MEMORY: Mutex<HashMap<u32, Device>> = Mutex::new(HashMap::new());
    pub static ref JOBS_IN_MEMORY: Mutex<HashMap<u32, Job>> = Mutex::new(HashMap::new());
    pub static ref MACROS_IN_MEMORY: Mutex<HashMap<u32, Macro>> = Mutex::new(HashMap::new());
    pub static ref SENSOR_DATA_IN_MEMORY: Mutex<HashMap<u32, SensorData>> = Mutex::new(HashMap::new());
    pub static ref LOG_EVENTS_IN_MEMORY: Mutex<HashMap<u32, LogEvent>> = Mutex::new(HashMap::new());
    pub static ref COMMANDS_IN_MEMORY: Mutex<HashMap<u32, Command>> = Mutex::new(HashMap::new());
}


/// Enum per rappresentare le diverse strategie di allocazione della memoria.
#[derive(Debug,Clone)]
pub enum AllocationStrategy {
    Standard,
    PoolBased,
    CustomEmbedded,
}

/// `MemoryManager` gestisce l'allocazione e la deallocazione della memoria in base alla strategia
/// selezionata dall'applicazione.
///
/// # Campi
/// - `default_allocation_strategy`: La strategia di allocazione utilizzata.
/// - `pool`: Un pool di buffer pre-allocati (usato solo nella strategia `PoolBased`).
/// - `memory_config`: Configurazione della memoria di default fornita dall'utente.
pub struct MemoryManager {
    default_allocation_strategy: AllocationStrategy,
    pool: Option<VecDeque<Box<[u8]>>>, // Pool per l'allocazione basata su pool
    memory_config: MemoryConfig,  // Configurazione della memoria di default 
}

impl MemoryManager {
    /// Crea un nuovo gestore della memoria per l'applicazione specificata.
    ///
    /// # Parametri
    /// - `app_type`: Il tipo di applicazione (ad esempio, `WebApp`, `ApiBackend`, `DesktopApp`, ecc.).
    /// - `memory_config`: La configurazione della memoria che specifica le dimensioni dei buffer e del pool.
    ///
    /// # Ritorna
    /// Un'istanza di `MemoryManager` o un errore di tipo `CoreError` in caso di fallimento.
    pub fn new(app_type: ApplicationType, memory_config: MemoryConfig) -> Result<Self, CoreError> {
        info!("Inizializzazione del MemoryManager...");

        // Determina la strategia di allocazione in base al tipo di applicazione.
        let strategy = match app_type {
            ApplicationType::WebApp | ApplicationType::ApiBackend => AllocationStrategy::PoolBased,
            ApplicationType::DesktopApp => AllocationStrategy::Standard,
            ApplicationType::AutomationScript => AllocationStrategy::Standard,
            ApplicationType::EmbeddedSystem => AllocationStrategy::CustomEmbedded,
            _ => {
                return Err(CoreError::ConfigurationError("Tipo di applicazione non supportato considera implementazione".to_string()));
            },
        };

        // Inizializza il pool solo se la strategia è `PoolBased`, utilizzando il `pool_size` configurato.
        let pool = if let AllocationStrategy::PoolBased = strategy {
            // Calcola quanti buffer servono in base alla dimensione totale del pool e del buffer
            let buffer_count = memory_config.pool_size / memory_config.buffer_size;
            let buffers = (0..buffer_count)
                .map(|_| vec![0u8; memory_config.buffer_size].into_boxed_slice())
                .collect::<VecDeque<_>>();
            Some(buffers)
        } else {
            None
        };

        Ok(Self { default_allocation_strategy: strategy, pool, memory_config })
    }

    /// Alloca memoria in base alla strategia configurata.
    ///
    /// # Parametri
    /// - `strategy`: La strategia di allocazione opzionale (`AllocationStrategy`). Se `None`, verrà utilizzata la strategia di default.
    /// - `size`: La quantità di memoria da allocare in byte.
    ///
    /// # Ritorna
    /// Un buffer di memoria (`Box<[u8]>`) o un errore di tipo `CoreError` in caso di fallimento.
    ///
    /// # Nota
    /// - La strategia `Standard` alloca dinamicamente la memoria.
    /// - La strategia `PoolBased` utilizza buffer pre-allocati dal pool. Se il pool è esaurito, viene effettuata un'allocazione dinamica.
    /// - La strategia `CustomEmbedded` utilizza una configurazione fissa per i buffer, che è specificata dalla configurazione della memoria (`memory_config`).
    pub fn allocate(&mut self, strategy: Option<AllocationStrategy>, size: usize) -> Result<Box<[u8]>, CoreError> {
        let alloc_strategy = strategy.unwrap_or(self.default_allocation_strategy.clone());
    
        info!("Allocazione di {} byte di memoria con strategia {:?}...", size, alloc_strategy);
        match alloc_strategy {
            AllocationStrategy::Standard => {
                let buffer = vec![0u8; size].into_boxed_slice();
                Ok(buffer)
            },
            AllocationStrategy::PoolBased => {
                if let Some(ref mut pool) = self.pool {
                    if let Some(buffer) = pool.pop_front() {
                        Ok(buffer)
                    } else {
                        // Pool esaurito, alloca dinamicamente
                        let buffer = vec![0u8; size].into_boxed_slice();
                        Ok(buffer)
                    }
                } else {
                    Err(CoreError::ResourceAllocationError("Pool non disponibile".to_string()))
                }
            },
            AllocationStrategy::CustomEmbedded => {
                // Usa la dimensione configurata per i buffer negli embedded.
                let buffer = vec![0u8; self.memory_config.buffer_size].into_boxed_slice();
                Ok(buffer)
            },
        }
    }
    

    /// Dealloca memoria precedentemente allocata.
    ///
    /// # Parametri
    /// - `buffer`: Il buffer di memoria da deallocare.
    ///
    /// # Ritorna
    /// `Ok(())` se la deallocazione ha successo, oppure un errore di tipo `CoreError`.
    ///
    /// # Nota
    /// - Nella strategia `Standard`, Rust dealloca automaticamente la memoria.
    /// - Nella strategia `PoolBased`, il buffer viene restituito al pool.
    /// - Nella strategia `CustomEmbedded`, non è richiesta alcuna azione specifica.
    pub fn deallocate(&mut self, buffer: Box<[u8]>) -> Result<(), CoreError> {
        info!("Deallocazione della memoria...");
        match self.default_allocation_strategy {
            AllocationStrategy::Standard => {
                // Rust dealloca automaticamente la memoria.
                Ok(())
            },
            AllocationStrategy::PoolBased => {
                // Restituisce il buffer al pool.
                if let Some(ref mut pool) = self.pool {
                    pool.push_back(buffer);
                    Ok(())
                } else {
                    Err(CoreError::ResourceAllocationError("Pool non disponibile".to_string()))
                }
            },
            AllocationStrategy::CustomEmbedded => {
                // Gestione personalizzata per sistemi embedded.
                Ok(())
            },
        }
    }
}


fn usize_max_value(var_name: &str) -> usize {
    println!("Il valore di {} eccede il limite massimo di usize.\n\
    Vuoi assegnare il valore massimo consentito ({} /2)? [y/n]", var_name, usize::MAX );

    let mut input = String::new();
    io::stdout().flush().unwrap(); // Assicura che il prompt venga mostrato
    io::stdin().read_line(&mut input).expect("Errore nella lettura dell'input");

    match input.trim().to_lowercase().as_str() {
        "y" => usize::MAX/2,
        "n" => {
            println!("Inserisci un nuovo valore:");
            input.clear();
            io::stdin().read_line(&mut input).expect("Errore nella lettura dell'input");
            
            match input.trim().parse::<usize>() {
                Ok(val) => val,
                Err(_) => {
                    println!("Input non valido. Riprova.");
                    usize_max_value(var_name)
                }
            }
        }
        _ => {
            println!("Input non valido. Digita 'y' per accettare il valore \
            massimo o 'n' per inserire un nuovo valore.");
            usize_max_value(var_name)
        }
    }
}

// Calcola il buffer_size
pub fn define_buffer_size(app_type: ApplicationType, buffer_size: usize) -> usize {

    if buffer_size > usize::MAX/2 {
        return usize_max_value("buffer_size");
    } else if buffer_size != 0 {
        return buffer_size;
    }

    match app_type {
        ApplicationType::WebApp => 16 * 1024 * 1024, // 16 MB
        ApplicationType::ApiBackend => 8 * 1024 * 1024, // 8 MB
        ApplicationType::DesktopApp => 4 * 1024 * 1024, // 4 MB
        ApplicationType::AutomationScript => 2 * 1024 * 1024, // 2 MB
        ApplicationType::EmbeddedSystem => 512 * 1024, // 512 KB
        _ => 0,
    }
}

// Calcola il pool_size
pub fn define_pool_size(app_type: ApplicationType, pool_size: usize) -> usize {

    if pool_size > usize::MAX/2 {
        return usize_max_value("pool_size");
    } else if pool_size != 0 {
        return pool_size;
    }

    match app_type {
        ApplicationType::WebApp => 150 * 1024 * 1024, // 150 MB
        ApplicationType::ApiBackend => 100 * 1024 * 1024, // 100 MB
        ApplicationType::DesktopApp => 50 * 1024 * 1024, // 50 MB
        ApplicationType::AutomationScript => 30 * 1024 * 1024, // 30 MB
        ApplicationType::EmbeddedSystem => 5 * 1024 * 1024, // 5 MB
        _ => 0,
    }
}

pub fn define_multiplier(app_type: ApplicationType, memory_scale: u8) -> u8 {
    if memory_scale > u8::MAX {
        println!("Il valore di memory_scale eccede il limite massimo di u8.\n\
        Vuoi assegnare il valore massimo consentito ({})? [y/n]", u8::MAX);

        let mut input = String::new();
        io::stdout().flush().unwrap(); // Assicura che il prompt venga mostrato
        io::stdin().read_line(&mut input).expect("Errore nella lettura dell'input");

        match input.trim().to_lowercase().as_str() {
            "y" => return u8::MAX,
            "n" => {
                println!("Inserisci un nuovo valore:");
                input.clear();
                io::stdin().read_line(&mut input).expect("Errore nella lettura dell'input");
                
                return match input.trim().parse::<u8>() {
                    Ok(val) => val,
                    Err(_) => {
                        println!("Input non valido. Riprova.");
                        return define_multiplier(app_type, memory_scale);
                    }
                };
            }
            _ => {
                println!("Input non valido. Digita 'y' per accettare il valore \
                massimo o 'n' per inserire un nuovo valore.");
                return define_multiplier(app_type, memory_scale);
            }
        }
    } else if memory_scale != 0 {
        return memory_scale;
    }

    match app_type {
        ApplicationType::WebApp => 1,
        ApplicationType::ApiBackend => 1,
        ApplicationType::DesktopApp => 1,
        ApplicationType::AutomationScript => 1,
        ApplicationType::EmbeddedSystem => 1,
        _ => 0,
    }
}

// # Aggiunta di Nuovi Modelli o Strategie di Allocazione
// Se desideri aggiungere nuovi modelli o strategie di allocazione, segui questi passaggi:
// 1. **Nuova Strategia**: Aggiungi una nuova variante all'enum `AllocationStrategy` per rappresentare la tua strategia.
// 2. **Logica di Allocazione**: Aggiorna i metodi `allocate` e `deallocate` per gestire la nuova strategia.
// 3. **Configurazione**: Assicurati di aggiungere nuove opzioni alla struttura `MemoryConfig` per configurare il comportamento della nuova strategia.
// 4. **Testing**: Scrivi test unitari per verificare il corretto funzionamento della tua nuova strategia di allocazione.
