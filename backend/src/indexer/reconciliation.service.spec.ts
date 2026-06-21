import { Test, TestingModule } from '@nestjs/testing';
import { ConfigService } from '@nestjs/config';
import { getRepositoryToken } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { ReconciliationService } from './reconciliation.service';
import { ChainSyncCheckpoint } from './entities/chain-sync-checkpoint.entity';
import { ContractEvent } from './entities/contract-event.entity';

describe('ReconciliationService', () => {
  let service: ReconciliationService;
  let checkpointRepository: jest.Mocked<
    Pick<Repository<ChainSyncCheckpoint>, 'findOne' | 'create' | 'save'>
  >;
  let contractEventRepository: jest.Mocked<
    Pick<Repository<ContractEvent>, 'findOne' | 'create' | 'save'>
  >;
  let configService: jest.Mocked<Pick<ConfigService, 'get'>>;

  beforeEach(async () => {
    checkpointRepository = {
      findOne: jest.fn(),
      create: jest.fn(),
      save: jest.fn(),
    };

    contractEventRepository = {
      findOne: jest.fn(),
      create: jest.fn(),
      save: jest.fn(),
    };

    configService = {
      get: jest.fn(),
    };

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        ReconciliationService,
        { provide: ConfigService, useValue: configService },
        {
          provide: getRepositoryToken(ChainSyncCheckpoint),
          useValue: checkpointRepository,
        },
        {
          provide: getRepositoryToken(ContractEvent),
          useValue: contractEventRepository,
        },
      ],
    }).compile();

    service = module.get<ReconciliationService>(ReconciliationService);
  });

  it('should be defined', () => {
    expect(service).toBeDefined();
  });

  describe('isEnabled', () => {
    it('returns true by default', () => {
      configService.get.mockReturnValue(undefined);
      expect(service.isEnabled()).toBe(true);
    });

    it('returns false when RECONCILE_ENABLED is "false"', () => {
      configService.get.mockReturnValue('false');
      expect(service.isEnabled()).toBe(false);
    });

    it('returns false when RECONCILE_ENABLED is "0"', () => {
      configService.get.mockReturnValue('0');
      expect(service.isEnabled()).toBe(false);
    });

    it('returns true when RECONCILE_ENABLED is "true"', () => {
      configService.get.mockReturnValue('true');
      expect(service.isEnabled()).toBe(true);
    });
  });

  describe('getReconcileWindow', () => {
    it('returns default window when not configured', () => {
      configService.get.mockReturnValue(undefined);
      expect(service.getReconcileWindow()).toBe(200);
    });

    it('returns configured window', () => {
      configService.get.mockReturnValue(500);
      expect(service.getReconcileWindow()).toBe(500);
    });
  });

  describe('getReconcileIntervalMs', () => {
    it('returns default interval when not configured', () => {
      configService.get.mockReturnValue(undefined);
      expect(service.getReconcileIntervalMs()).toBe(60000);
    });

    it('returns configured interval', () => {
      configService.get.mockReturnValue(30000);
      expect(service.getReconcileIntervalMs()).toBe(30000);
    });
  });

  describe('advanceCheckpoint', () => {
    it('creates a checkpoint if none exists and advances it', async () => {
      checkpointRepository.findOne.mockResolvedValue(null);
      checkpointRepository.create.mockReturnValue({
        contract_id: 'CTEST',
        last_indexed_ledger: 0,
        chain_head_ledger: 0,
      } as ChainSyncCheckpoint);
      checkpointRepository.save.mockImplementation(
        async (cp) => cp as ChainSyncCheckpoint,
      );

      await service.advanceCheckpoint('CTEST', 100);

      expect(checkpointRepository.save).toHaveBeenCalledWith(
        expect.objectContaining({
          contract_id: 'CTEST',
          last_indexed_ledger: 100,
        }),
      );
    });

    it('does not regress the checkpoint', async () => {
      const existing = {
        contract_id: 'CTEST',
        last_indexed_ledger: 200,
        chain_head_ledger: 200,
      } as ChainSyncCheckpoint;

      checkpointRepository.findOne.mockResolvedValue(existing);

      await service.advanceCheckpoint('CTEST', 100);

      expect(checkpointRepository.save).not.toHaveBeenCalled();
    });

    it('advances the checkpoint when ledger is higher', async () => {
      const existing = {
        contract_id: 'CTEST',
        last_indexed_ledger: 100,
        chain_head_ledger: 100,
      } as ChainSyncCheckpoint;

      checkpointRepository.findOne.mockResolvedValue(existing);
      checkpointRepository.save.mockImplementation(
        async (cp) => cp as ChainSyncCheckpoint,
      );

      await service.advanceCheckpoint('CTEST', 200);

      expect(checkpointRepository.save).toHaveBeenCalledWith(
        expect.objectContaining({ last_indexed_ledger: 200 }),
      );
    });
  });

  describe('getStatus', () => {
    it('returns current reconciliation status', () => {
      configService.get.mockReturnValue(undefined);
      const status = service.getStatus();

      expect(status).toEqual({
        enabled: true,
        is_running: false,
        last_run_at: null,
        last_backfill_count: 0,
      });
    });
  });

  describe('getCheckpointForContract', () => {
    it('returns checkpoint when it exists', async () => {
      const checkpoint = {
        contract_id: 'CTEST',
        last_indexed_ledger: 500,
        chain_head_ledger: 600,
      } as ChainSyncCheckpoint;

      checkpointRepository.findOne.mockResolvedValue(checkpoint);

      const result = await service.getCheckpointForContract('CTEST');
      expect(result).toEqual(checkpoint);
    });

    it('returns null when no checkpoint exists', async () => {
      checkpointRepository.findOne.mockResolvedValue(null);

      const result = await service.getCheckpointForContract('CTEST');
      expect(result).toBeNull();
    });
  });

  describe('reconcile', () => {
    it('skips when disabled', async () => {
      configService.get.mockImplementation((key: string) => {
        if (key === 'RECONCILE_ENABLED') return 'false';
        return undefined;
      });

      await service.reconcile();

      expect(checkpointRepository.findOne).not.toHaveBeenCalled();
    });

    it('skips when no contract ID configured', async () => {
      configService.get.mockImplementation((key: string) => {
        if (key === 'RECONCILE_ENABLED') return 'true';
        if (key === 'SOROBAN_CONTRACT_ID') return undefined;
        return undefined;
      });

      await service.reconcile();

      expect(checkpointRepository.findOne).not.toHaveBeenCalled();
    });

    it('skips when contract ID is placeholder', async () => {
      configService.get.mockImplementation((key: string) => {
        if (key === 'RECONCILE_ENABLED') return 'true';
        if (key === 'SOROBAN_CONTRACT_ID') return 'your-contract-id-here';
        return undefined;
      });

      await service.reconcile();

      expect(checkpointRepository.findOne).not.toHaveBeenCalled();
    });
  });
});
