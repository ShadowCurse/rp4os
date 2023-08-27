use crate::{
    bsp::memory::mmu::MSKernel,
    info,
    memory::{
        mmu::{AccessPermissions, AttributeFields, MMIODescriptor, MemAttributes, MemoryRegion},
        Address, Physical, Virtual,
    },
    size_human_readable_ceil, synchronization,
    synchronization::InitStateLock,
    warn,
};
use synchronization::ReadWriteExclusive;

const MAX_MAPPINGS: usize = 20;

static KERNEL_MAPPING_RECORDS: InitStateLock<MappingRecords> =
    InitStateLock::new(MappingRecords::new());

/// Add an entry to the mapping info record.
pub fn kernel_add_mapping_record(
    name: &'static str,
    virt_region: &MemoryRegion<Virtual>,
    phys_region: &MemoryRegion<Physical>,
    attr: &AttributeFields,
) -> Result<(), &'static str> {
    KERNEL_MAPPING_RECORDS.write(|records| records.add(name, virt_region, phys_region, attr))
}

/// Tries to add device as a user to the existing record.
pub fn kernel_try_add_device_record_mmio_user(
    new_user: &'static str,
    mmio_descriptor: &MMIODescriptor,
) -> Option<Address<Virtual>> {
    let phys_region: MemoryRegion<Physical> = (*mmio_descriptor).into();

    KERNEL_MAPPING_RECORDS.write(|records| {
        let record = records.find_device_record(&phys_region)?;

        if let Err(x) = record.add_user(new_user) {
            warn!("{}", x);
        }

        Some(record.virt_start_addr)
    })
}

/// Human-readable print of all recorded kernel mappings.
pub fn print_kernel_mappings() {
    KERNEL_MAPPING_RECORDS.read(|mr| mr.print());
}

/// Type describing a virtual memory mapping.
#[derive(Copy, Clone)]
struct MappingRecordEntry {
    pub users: [Option<&'static str>; 5],
    pub phys_start_addr: Address<Physical>,
    pub virt_start_addr: Address<Virtual>,
    pub num_pages: usize,
    pub attribute_fields: AttributeFields,
}

struct MappingRecords {
    inner: [Option<MappingRecordEntry>; MAX_MAPPINGS],
}

impl MappingRecordEntry {
    pub fn new(
        name: &'static str,
        virt_region: &MemoryRegion<Virtual>,
        phys_region: &MemoryRegion<Physical>,
        attr: &AttributeFields,
    ) -> Self {
        Self {
            users: [Some(name), None, None, None, None],
            phys_start_addr: phys_region.start_page.address(),
            virt_start_addr: virt_region.start_page.address(),
            num_pages: phys_region.num_pages(),
            attribute_fields: *attr,
        }
    }

    fn find_next_free_user(&mut self) -> Result<&mut Option<&'static str>, &'static str> {
        if let Some(x) = self.users.iter_mut().find(|x| x.is_none()) {
            return Ok(x);
        };

        Err("Storage for user info exhausted")
    }

    pub fn add_user(&mut self, user: &'static str) -> Result<(), &'static str> {
        let x = self.find_next_free_user()?;
        *x = Some(user);
        Ok(())
    }
}

impl MappingRecords {
    pub const fn new() -> Self {
        Self {
            inner: [None; MAX_MAPPINGS],
        }
    }

    fn size(&self) -> usize {
        self.inner.iter().filter(|x| x.is_some()).count()
    }

    fn sort(&mut self) {
        let upper_bound_exclusive = self.size();
        let entries = &mut self.inner[0..upper_bound_exclusive];

        if !entries.is_sorted_by_key(|item| item.unwrap().virt_start_addr) {
            entries.sort_unstable_by_key(|item| item.unwrap().virt_start_addr)
        }
    }

    fn find_next_free(&mut self) -> Result<&mut Option<MappingRecordEntry>, &'static str> {
        if let Some(x) = self.inner.iter_mut().find(|x| x.is_none()) {
            return Ok(x);
        }

        Err("Storage for mapping info exhausted")
    }

    fn find_device_record(
        &mut self,
        phys_region: &MemoryRegion<Physical>,
    ) -> Option<&mut MappingRecordEntry> {
        self.inner
            .iter_mut()
            .filter_map(|x| x.as_mut())
            .filter(|x| x.attribute_fields.mem_attributes == MemAttributes::Device)
            .find(|x| {
                if x.phys_start_addr != phys_region.start_page.address() {
                    return false;
                }

                if x.num_pages != phys_region.num_pages() {
                    return false;
                }

                true
            })
    }

    pub fn add(
        &mut self,
        name: &'static str,
        virt_region: &MemoryRegion<Virtual>,
        phys_region: &MemoryRegion<Physical>,
        attr: &AttributeFields,
    ) -> Result<(), &'static str> {
        let x = self.find_next_free()?;

        *x = Some(MappingRecordEntry::new(
            name,
            virt_region,
            phys_region,
            attr,
        ));

        self.sort();

        Ok(())
    }

    pub fn print(&self) {
        info!("      -------------------------------------------------------------------------------------------------------------------------------------------");
        info!(
            "      {:^44}     {:^30}   {:^7}   {:^9}   {:^35}",
            "Virtual", "Physical", "Size", "Attr", "Entity"
        );
        info!("      -------------------------------------------------------------------------------------------------------------------------------------------");

        for i in self.inner.iter().flatten() {
            let size = i.num_pages * MSKernel::SIZE;
            let virt_start = i.virt_start_addr;
            let virt_end_inclusive = virt_start + (size - 1);
            let phys_start = i.phys_start_addr;
            let phys_end_inclusive = phys_start + (size - 1);

            let (size, unit) = size_human_readable_ceil(size);

            let attr = match i.attribute_fields.mem_attributes {
                MemAttributes::CacheableDRAM => "Cache",
                MemAttributes::Device => "Device",
            };

            let acc_p = match i.attribute_fields.acc_perms {
                AccessPermissions::ReadOnly => "RO",
                AccessPermissions::ReadWrite => "RW",
            };

            let xn = if i.attribute_fields.execute_never {
                "XN"
            } else {
                "X"
            };

            info!(
                "      {}..{} --> {}..{} | {:>3} {} | {:<3} {} {:<2} | {}",
                virt_start,
                virt_end_inclusive,
                phys_start,
                phys_end_inclusive,
                size,
                unit,
                attr,
                acc_p,
                xn,
                i.users[0].unwrap()
            );

            for k in i.users[1..].iter() {
                if let Some(additional_user) = *k {
                    info!(
                        "                                                                                                            | {}",
                        additional_user
                    );
                }
            }
        }

        info!("      -------------------------------------------------------------------------------------------------------------------------------------------");
    }
}
