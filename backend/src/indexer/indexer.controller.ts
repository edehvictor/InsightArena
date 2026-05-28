import {
  Controller,
  Get,
  Post,
  Query,
  Body,
  UseGuards,
  UseInterceptors,
} from '@nestjs/common';
import { CacheInterceptor, CacheTTL } from '@nestjs/cache-manager';
import {
  ApiBearerAuth,
  ApiOperation,
  ApiResponse,
  ApiTags,
} from '@nestjs/swagger';
import { Roles } from '../common/decorators/roles.decorator';
import { Role } from '../common/enums/role.enum';
import { JwtAuthGuard } from '../common/guards/jwt-auth.guard';
import { RolesGuard } from '../common/guards/roles.guard';
import { IndexerService } from './indexer.service';
import { ReindexDto, ReindexQueryDto } from './dto/reindex.dto';
import { IndexerMetricsDto } from './dto/indexer-metrics.dto';

@ApiTags('Indexer')
@Controller('admin/indexer')
@UseGuards(JwtAuthGuard, RolesGuard)
@Roles(Role.Admin)
@ApiBearerAuth()
export class IndexerController {
  constructor(private readonly indexerService: IndexerService) {}

  @Post('reindex')
  @ApiOperation({ summary: 'Trigger reindexing from a specific ledger' })
  @ApiResponse({ status: 200, description: 'Reindex triggered' })
  async reindex(@Body() dto: ReindexDto): Promise<{ message: string }> {
    await this.indexerService.reindex(dto.from_ledger);
    return {
      message: `Reindex triggered from ledger ${dto.from_ledger}`,
    };
  }

  @Get('events')
  @ApiOperation({
    summary: 'Get raw contract events with cursor-based pagination',
  })
  @ApiResponse({
    status: 200,
    description: 'Paginated contract events',
  })
  @UseInterceptors(CacheInterceptor)
  @CacheTTL(30)
  async getEvents(@Query() query: ReindexQueryDto) {
    return this.indexerService.getEventsPaginated(
      query.cursor,
      query.limit ?? 50,
    );
  }

  @Get('metrics')
  @ApiOperation({ summary: 'Get indexer health metrics' })
  @ApiResponse({
    status: 200,
    description: 'Indexer metrics',
    type: IndexerMetricsDto,
  })
  async getMetrics(): Promise<IndexerMetricsDto> {
    return this.indexerService.getMetrics();
  }

  @Post('retry')
  @ApiOperation({ summary: 'Retry all failed/DLQ events' })
  @ApiResponse({ status: 200, description: 'Retry initiated' })
  async retryFailed(): Promise<{ retried: number }> {
    const count = await this.indexerService.retryFailedEvents();
    return { retried: count };
  }
}
