// Fixed product capabilities for DPI stage-table commands.
const AUDITED_CAPABILITIES: &[DpiStagesDescriptor] = &[
    DpiStagesDescriptor {
        product_id: 182,
        min_dpi: 100,
        max_dpi: 30000,
        dpi_step: 50,
    },
    DpiStagesDescriptor {
        product_id: 170,
        min_dpi: 100,
        max_dpi: 30000,
        dpi_step: 50,
    },
    DpiStagesDescriptor {
        product_id: 204,
        min_dpi: 100,
        max_dpi: 35000,
        dpi_step: 50,
    },
    DpiStagesDescriptor {
        product_id: 226,
        min_dpi: 100,
        max_dpi: 50000,
        dpi_step: 1,
    },
    DpiStagesDescriptor {
        product_id: 45066,
        min_dpi: 100,
        max_dpi: 35000,
        dpi_step: 50,
    },
];
